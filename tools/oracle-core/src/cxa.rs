//! C++ exception handling for emscripten modules.
//!
//! Emscripten compiles C++ exceptions to calls into its JS glue: a throw
//! becomes a JS throw that unwinds to the nearest `invoke_*` trampoline, and the
//! landing pad then asks `__cxa_find_matching_catch_*` whether it can handle
//! what was thrown.
//!
//! With these stubbed to zero, every `try`/`catch` in the module is broken: the
//! catch never matches, so a recoverable error propagates out as a hard trap and
//! the caller sees a wasm backtrace instead of a result. Implementing them makes
//! ordinary C++ control flow work, and makes a genuinely thrown exception
//! readable instead of anonymous.
//!
//! The in-memory layout of an exception is emscripten's, not the C++ ABI's: a
//! 24-byte header sits immediately *before* the thrown object.

use anyhow::Result;
use wasmtime::error::Context as _;
use wasmtime::{Caller, Linker, Module, Store, Val};

use crate::state::HostState;

/// Size of the header emscripten places before a thrown object.
const HEADER: u32 = 24;

/// Field offsets within that header.
const REFCOUNT_OFFSET: u32 = 0;
const TYPE_OFFSET: u32 = 4;
const CAUGHT_OFFSET: u32 = 12;
const ADJUSTED_PTR_OFFSET: u32 = 16;

/// The exception currently in flight, mirroring emscripten's `exceptionLast`.
#[derive(Debug, Clone, Copy, Default)]
pub struct InFlight {
    /// Address of the thrown object.
    pub ptr: u32,
    /// Its type id, as `__cxa_throw` was given it.
    pub type_id: u32,
}

/// Reads the type id recorded in an exception's header.
fn exception_type(state: &HostState, ptr: u32) -> u32 {
    if ptr < HEADER {
        return 0;
    }
    state.read_u32(ptr - HEADER + TYPE_OFFSET).unwrap_or(0)
}

/// Extracts a human-readable message via the module's own
/// `__get_exception_message`, so a thrown exception is identified by what it
/// says rather than by a table index.
fn message_of(caller: &mut Caller<'_, HostState>, ptr: u32) -> Option<String> {
    let get_message = crate::exports::optional_func(
        caller,
        &["__get_exception_message", "_get_exception_message"],
    )?;
    let malloc = crate::exports::optional_func(caller, &["malloc", "_malloc"])?;

    // The accessor writes a (type, message) pair of char* into an out-param.
    let mut out = vec![Val::I32(0)];
    malloc.call(&mut *caller, &[Val::I32(8)], &mut out).ok()?;
    let Some(Val::I32(buffer)) = out.first().copied() else {
        return None;
    };
    if buffer == 0 {
        return None;
    }

    get_message
        .call(
            &mut *caller,
            &[Val::I32(ptr as i32), Val::I32(buffer), Val::I32(buffer + 4)],
            &mut [],
        )
        .ok()?;

    let state = caller.data();
    let type_name = state
        .read_u32(buffer as u32)
        .ok()
        .and_then(|ptr| state.read_cstr(ptr).ok())
        .unwrap_or_default();
    let text = state
        .read_u32(buffer as u32 + 4)
        .ok()
        .and_then(|ptr| state.read_cstr(ptr).ok())
        .unwrap_or_default();

    if let Some(free) = crate::exports::optional_func(caller, &["free", "_free"]) {
        let _ = free.call(&mut *caller, &[Val::I32(buffer)], &mut []);
    }

    match (type_name.is_empty(), text.is_empty()) {
        (true, true) => None,
        (false, true) => Some(type_name),
        (true, false) => Some(text),
        (false, false) => Some(format!("{type_name}: {text}")),
    }
}

/// Records a thrown exception and unwinds, which is what the `invoke_*`
/// trampoline is waiting for.
fn throw(caller: &mut Caller<'_, HostState>, ptr: u32, type_id: u32) -> wasmtime::Error {
    // Emscripten writes the type into the header before unwinding; later
    // matching reads it back from there.
    if ptr >= HEADER {
        let bytes = type_id.to_le_bytes();
        let _ = caller.data().write(ptr - HEADER + TYPE_OFFSET, &bytes);
    }

    let described = message_of(caller, ptr);
    caller.data_mut().in_flight = InFlight { ptr, type_id };

    match described {
        Some(message) => {
            caller.data().log(format!("C++ exception: {message}"));
            wasmtime::Error::msg(format!("C++ exception: {message}"))
        }
        None => wasmtime::Error::msg(format!("C++ exception at {ptr:#x} (type {type_id})")),
    }
}

/// Sets the module's `tempRet0`, the side channel emscripten's landing pads read
/// the matched type id from.
fn set_temp_ret0(caller: &mut Caller<'_, HostState>, value: u32) {
    if let Some(func) = crate::exports::optional_func(caller, &["setTempRet0", "_setTempRet0"]) {
        let _ = func.call(&mut *caller, &[Val::I32(value as i32)], &mut []);
    }
}

/// Implements `__cxa_find_matching_catch_*`: decides whether the in-flight
/// exception is caught by any of the candidate types the landing pad offers.
fn find_matching_catch(caller: &mut Caller<'_, HostState>, candidates: &[u32]) -> i32 {
    let thrown = caller.data().in_flight;
    if thrown.ptr == 0 {
        set_temp_ret0(caller, 0);
        return 0;
    }

    // The adjusted pointer defaults to the thrown object; __cxa_can_catch may
    // rewrite it when catching a base class of what was thrown.
    let header = thrown.ptr.saturating_sub(HEADER);
    let _ = caller
        .data()
        .write(header + ADJUSTED_PTR_OFFSET, &thrown.ptr.to_le_bytes());

    let thrown_type = match exception_type(caller.data(), thrown.ptr) {
        0 => thrown.type_id,
        recorded => recorded,
    };
    if thrown_type == 0 {
        set_temp_ret0(caller, 0);
        return thrown.ptr as i32;
    }

    let can_catch = crate::exports::optional_func(caller, &["__cxa_can_catch", "_cxa_can_catch"]);

    for &candidate in candidates {
        // A null candidate is the `catch (...)` clause: it matches anything.
        if candidate == 0 || candidate == thrown_type {
            set_temp_ret0(caller, candidate);
            return thrown.ptr as i32;
        }

        if let Some(func) = can_catch {
            let mut results = vec![Val::I32(0)];
            let args = [
                Val::I32(candidate as i32),
                Val::I32(thrown_type as i32),
                Val::I32((header + ADJUSTED_PTR_OFFSET) as i32),
            ];
            if func.call(&mut *caller, &args, &mut results).is_ok()
                && matches!(results.first(), Some(Val::I32(matched)) if *matched != 0)
            {
                set_temp_ret0(caller, candidate);
                let adjusted = caller
                    .data()
                    .read_u32(header + ADJUSTED_PTR_OFFSET)
                    .unwrap_or(thrown.ptr);
                return adjusted as i32;
            }
        }
    }

    set_temp_ret0(caller, thrown_type);
    thrown.ptr as i32
}

/// Defines the exception-handling imports the module declares.
///
/// Each is built from the module's own signature, because the number of
/// candidate types in `__cxa_find_matching_catch_N` is encoded in the name and
/// the arity varies with it.
pub fn define(
    store: &mut Store<HostState>,
    linker: &mut Linker<HostState>,
    module: &Module,
) -> Result<usize> {
    let mut defined = 0;

    for import in module.imports() {
        let wasmtime::ExternType::Func(ty) = import.ty() else {
            continue;
        };
        let name = import.name();

        let func = if name == "__cxa_throw" {
            // (object, type, destructor)
            crate::host::host_func(&mut *store, ty.clone(), |caller, params, _results| {
                let ptr = int_arg(params, 0);
                let type_id = int_arg(params, 1);
                Err(throw(caller, ptr, type_id))
            })
        } else if name == "__cxa_rethrow" || name == "__resumeException" {
            let takes_pointer = ty.params().len() > 0;
            crate::host::host_func(&mut *store, ty.clone(), move |caller, params, _results| {
                let ptr = if takes_pointer {
                    int_arg(params, 0)
                } else {
                    caller.data().in_flight.ptr
                };
                let type_id = exception_type(caller.data(), ptr);
                Err(throw(caller, ptr, type_id))
            })
        } else if name == "__cxa_rethrow_primary_exception" {
            // `std::rethrow_exception`. Emscripten makes this the pointer-taking
            // form of `__cxa_rethrow`: it makes the argument current and
            // unwinds. A null argument is the one case that returns normally.
            //
            // Stubbed to a no-op it *always* returns normally, and since the
            // C++ declaration is `[[noreturn]]` libc++ answers a return by
            // calling `std::terminate` — whose handler, reached through
            // `invoke_v`, calls terminate again. That is a 10,000-frame stack
            // overflow with no diagnostic, and it is what this capture does in
            // half its startups without this.
            crate::host::host_func(&mut *store, ty.clone(), |caller, params, _results| {
                let ptr = int_arg(params, 0);
                if ptr == 0 {
                    return Ok(());
                }
                let type_id = exception_type(caller.data(), ptr);
                Err(throw(caller, ptr, type_id))
            })
        } else if name == "__cxa_current_primary_exception" {
            // `std::current_exception`. Hands back the exception in flight and
            // takes a reference to it, because the caller is storing it in an
            // `exception_ptr` that outlives the catch block.
            crate::host::host_func(&mut *store, ty.clone(), |caller, _params, results| {
                let ptr = caller.data().in_flight.ptr;
                if ptr >= HEADER {
                    let header = ptr - HEADER + REFCOUNT_OFFSET;
                    let count = caller.data().read_u32(header).unwrap_or(0);
                    let _ = caller
                        .data()
                        .write(header, &count.wrapping_add(1).to_le_bytes());
                }
                if let Some(slot) = results.first_mut() {
                    *slot = Val::I32(ptr as i32);
                }
                Ok(())
            })
        } else if let Some(count) = find_matching_catch_arity(name) {
            crate::host::host_func(&mut *store, ty.clone(), move |caller, params, results| {
                let candidates: Vec<u32> = (0..count.min(params.len()))
                    .map(|index| int_arg(params, index))
                    .collect();
                let matched = find_matching_catch(caller, &candidates);
                if let Some(slot) = results.first_mut() {
                    *slot = Val::I32(matched);
                }
                Ok(())
            })
        } else if name == "__cxa_begin_catch" {
            // Mark the exception as caught and hand back the object pointer.
            crate::host::host_func(&mut *store, ty.clone(), |caller, params, results| {
                let ptr = int_arg(params, 0);
                if ptr >= HEADER {
                    let _ = caller.data().write(ptr - HEADER + CAUGHT_OFFSET, &[1]);
                }
                if let Some(slot) = results.first_mut() {
                    *slot = Val::I32(ptr as i32);
                }
                Ok(())
            })
        } else if name == "__cxa_end_catch" {
            crate::host::host_func(&mut *store, ty.clone(), |caller, _params, _results| {
                caller.data_mut().in_flight = InFlight::default();
                Ok(())
            })
        } else if name == "__cxa_get_exception_ptr" || name == "llvm_eh_typeid_for" {
            // Both are identity in emscripten's model: the pointer *is* the
            // exception object, and a type pointer *is* its id.
            crate::host::host_func(&mut *store, ty.clone(), |_caller, params, results| {
                if let Some(slot) = results.first_mut() {
                    *slot = Val::I32(int_arg(params, 0) as i32);
                }
                Ok(())
            })
        } else if name == "__cxa_uncaught_exceptions" {
            crate::host::host_func(&mut *store, ty.clone(), |caller, _params, results| {
                if let Some(slot) = results.first_mut() {
                    *slot = Val::I32(i32::from(caller.data().in_flight.ptr != 0));
                }
                Ok(())
            })
        } else {
            continue;
        };

        linker
            .define(&*store, import.module(), import.name(), func)
            .with_context(|| format!("defining {name}"))?;
        defined += 1;
    }

    Ok(defined)
}

/// `__cxa_find_matching_catch_N` takes `N - 2` candidate types; the name encodes
/// the total including the two implicit slots.
fn find_matching_catch_arity(name: &str) -> Option<usize> {
    let suffix = name.strip_prefix("__cxa_find_matching_catch_")?;
    let total: usize = suffix.parse().ok()?;
    Some(total.saturating_sub(2))
}

fn int_arg(params: &[Val], index: usize) -> u32 {
    match params.get(index) {
        Some(Val::I32(value)) => *value as u32,
        Some(Val::I64(value)) => *value as u32,
        _ => 0,
    }
}
