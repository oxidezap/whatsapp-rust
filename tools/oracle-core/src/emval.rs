//! emscripten's `val` bridge.
//!
//! `emscripten::val` is a handle to a value living on the *JavaScript* side. C++
//! code that returns one hands back an opaque integer, and the glue keeps a
//! table mapping that integer to the real value. Without such a table any
//! function returning `val` is unreadable — which covers every `get` on a
//! registered vector, and so covers reading anything back out of a module.
//!
//! Only the three entry points the captured modules import are implemented:
//! `_emval_take_value`, `_emval_incref` and `_emval_decref`. Anything else is
//! absent rather than faked.

use std::collections::BTreeMap;

use anyhow::Result;
use wasmtime::error::Context as _;
use wasmtime::{Caller, Linker, Module, Store, Val};

use crate::call::Value;
use crate::state::HostState;

/// Values the guest has handed out as `val` handles.
#[derive(Debug, Default)]
pub struct EmvalTable {
    values: BTreeMap<u32, Entry>,
    next: u32,
}

#[derive(Debug)]
struct Entry {
    value: Value,
    refcount: u32,
}

impl EmvalTable {
    /// Stores a value and returns its handle.
    ///
    /// Handles start at 1 so that zero keeps its usual meaning of "no value".
    pub fn insert(&mut self, value: Value) -> u32 {
        self.next += 1;
        let handle = self.next;
        self.values.insert(handle, Entry { value, refcount: 1 });
        handle
    }

    /// The value behind a handle, if it is still live.
    #[must_use]
    pub fn get(&self, handle: u32) -> Option<&Value> {
        self.values.get(&handle).map(|entry| &entry.value)
    }

    /// Takes another reference to a handle.
    pub fn incref(&mut self, handle: u32) {
        if let Some(entry) = self.values.get_mut(&handle) {
            entry.refcount += 1;
        }
    }

    /// Drops a reference, removing the value when the last one goes.
    ///
    /// A handle the guest releases more times than it took is ignored rather
    /// than treated as an error: the reference counting here exists to bound
    /// the table, not to police the module.
    pub fn decref(&mut self, handle: u32) {
        let Some(entry) = self.values.get_mut(&handle) else {
            return;
        };
        entry.refcount = entry.refcount.saturating_sub(1);
        if entry.refcount == 0 {
            self.values.remove(&handle);
        }
    }

    /// How many handles are live.
    #[must_use]
    pub fn len(&self) -> usize {
        self.values.len()
    }

    /// Whether no handle is live.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.values.is_empty()
    }
}

/// How many bytes a registered type occupies when read through a pointer, and
/// how to interpret them.
///
/// `readValueFromPointer` in embind dispatches on the registered type; the type
/// name is the only description available here, so it is what decides.
fn read_through_pointer(state: &HostState, type_name: &str, ptr: u32) -> Option<Value> {
    let read_int = |width: u32, signed: bool| -> Option<Value> {
        let bytes = state.read(ptr, width).ok()?;
        let mut buf = [0u8; 8];
        buf[..bytes.len()].copy_from_slice(&bytes);
        let raw = u64::from_le_bytes(buf);
        Some(Value::Int(if signed {
            // Sign-extend from the type's own width.
            let shift = 64 - width * 8;
            ((raw << shift) as i64) >> shift
        } else {
            raw as i64
        }))
    };

    match type_name {
        "bool" => Some(Value::Bool(state.read(ptr, 1).ok()?[0] != 0)),
        "char" | "signed char" => read_int(1, true),
        "unsigned char" => read_int(1, false),
        "short" => read_int(2, true),
        "unsigned short" => read_int(2, false),
        "int" | "long" => read_int(4, true),
        "unsigned int" | "unsigned long" => read_int(4, false),
        "int64_t" | "long long" => read_int(8, true),
        "uint64_t" | "unsigned long long" => read_int(8, false),
        "float" => {
            let bytes = state.read(ptr, 4).ok()?;
            Some(Value::Double(
                f32::from_le_bytes(bytes.try_into().ok()?) as f64
            ))
        }
        "double" => {
            let bytes = state.read(ptr, 8).ok()?;
            Some(Value::Double(f64::from_le_bytes(bytes.try_into().ok()?)))
        }
        "std::string" | "std::basic_string<unsigned char>" => {
            // The pointer is to a pointer to the string block.
            let block = state.read_u32(ptr).ok()?;
            let len = state.read_u32(block).ok()?;
            let bytes = state.read(block + 4, len).ok()?;
            Some(Value::Str(String::from_utf8_lossy(&bytes).into_owned()))
        }
        _ => None,
    }
}

/// Defines the `_emval_*` imports the module declares.
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

        let func = match import.name() {
            "_emval_take_value" => crate::host::host_func(
                &mut *store,
                ty.clone(),
                |caller: &mut Caller<'_, HostState>, params, results| {
                    // (TYPEID type, void* argv) -> EM_VAL
                    let type_id = int_arg(params, 0);
                    let ptr = int_arg(params, 1);

                    let state = caller.data();
                    let type_name = state.embind.type_name(type_id);
                    let value = read_through_pointer(state, &type_name, ptr);

                    let handle = match value {
                        Some(value) => caller.data_mut().emval.insert(value),
                        // An unreadable type yields no handle rather than a
                        // handle to something invented.
                        None => {
                            caller.data().log(format!(
                                "_emval_take_value: no reader for type `{type_name}`"
                            ));
                            0
                        }
                    };
                    if let Some(slot) = results.first_mut() {
                        *slot = Val::I32(handle as i32);
                    }
                    Ok(())
                },
            ),

            "_emval_incref" => crate::host::host_func(
                &mut *store,
                ty.clone(),
                |caller: &mut Caller<'_, HostState>, params, _results| {
                    let handle = int_arg(params, 0);
                    caller.data_mut().emval.incref(handle);
                    Ok(())
                },
            ),

            "_emval_decref" => crate::host::host_func(
                &mut *store,
                ty.clone(),
                |caller: &mut Caller<'_, HostState>, params, _results| {
                    let handle = int_arg(params, 0);
                    caller.data_mut().emval.decref(handle);
                    Ok(())
                },
            ),

            _ => continue,
        };

        linker
            .define(&*store, import.module(), import.name(), func)
            .with_context(|| format!("defining {}", import.name()))?;
        defined += 1;
    }

    Ok(defined)
}

fn int_arg(params: &[Val], index: usize) -> u32 {
    match params.get(index) {
        Some(Val::I32(value)) => *value as u32,
        Some(Val::I64(value)) => *value as u32,
        _ => 0,
    }
}
