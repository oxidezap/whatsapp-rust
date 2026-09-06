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

use anyhow::{Result, ensure};
use wasmtime::error::Context as _;
use wasmtime::{Caller, Linker, Module, Store, Val};

use crate::call::Value;
use crate::state::HostState;

/// Fixed budget of live emval handles. Worker fuel is replenished at host
/// boundaries, so a looping guest could otherwise retain values forever.
const MAX_EMVAL_HANDLES: usize = 4096;
/// Cumulative retained payload bytes across live string handles.
const MAX_EMVAL_BYTES: usize = 64 * 1024 * 1024;

/// Values the guest has handed out as `val` handles.
#[derive(Debug, Default)]
pub struct EmvalTable {
    values: BTreeMap<u32, Entry>,
    bytes: usize,
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
    pub fn insert(&mut self, value: Value) -> Result<u32> {
        let size = value_size(&value);
        ensure!(
            self.values.len() < MAX_EMVAL_HANDLES,
            "emval handle budget exceeded"
        );
        ensure!(
            self.bytes.saturating_add(size) <= MAX_EMVAL_BYTES,
            "emval payload budget exceeded"
        );
        self.next += 1;
        let handle = self.next;
        self.bytes += size;
        self.values.insert(handle, Entry { value, refcount: 1 });
        Ok(handle)
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
        if entry.refcount == 0
            && let Some(entry) = self.values.remove(&handle)
        {
            self.bytes = self.bytes.saturating_sub(value_size(&entry.value));
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

/// Retained payload bytes of one value. Handles and scalars carry no guest
/// bytes; strings do, and each one can hold up to 16 MiB.
fn value_size(value: &Value) -> usize {
    match value {
        Value::Str(text) => text.len(),
        Value::Bytes(bytes) => bytes.len(),
        _ => 0,
    }
}

/// How many bytes a registered type occupies when read through a pointer, and
/// how to interpret them.
///
/// `readValueFromPointer` in embind dispatches on the registered type; the type
/// registration supplies integer width/signedness; known names cover primitives.
fn read_through_pointer(
    state: &HostState,
    type_id: u32,
    type_name: &str,
    ptr: u32,
) -> Option<Value> {
    if let Some(integer) = state
        .embind
        .integer_type(type_id)
        .or_else(|| crate::integer::IntegerType::named(type_name))
    {
        let bytes = state.read(ptr, u32::from(integer.bytes())).ok()?;
        let mut buffer = [0; 8];
        buffer[..bytes.len()].copy_from_slice(&bytes);
        return Some(integer.decode(u64::from_le_bytes(buffer)));
    }

    match type_name {
        "bool" => Some(Value::Bool(state.read(ptr, 1).ok()?[0] != 0)),
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
            let bytes = crate::call::read_string_bytes(state, block).ok()?;
            if type_name == "std::string" {
                Some(Value::Str(String::from_utf8(bytes).ok()?))
            } else {
                Some(Value::Bytes(bytes))
            }
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
                    let value = read_through_pointer(state, type_id, &type_name, ptr);

                    let value = value.ok_or_else(|| {
                        wasmtime::Error::msg(format!(
                            "_emval_take_value: invalid or unsupported value for {type_name}"
                        ))
                    })?;
                    let handle = caller
                        .data_mut()
                        .emval
                        .insert(value)
                        .map_err(wasmtime::Error::msg)?;
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

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn handle_and_payload_budgets_are_enforced() {
        let mut table = EmvalTable::default();
        let mut handles = Vec::new();
        for _ in 0..MAX_EMVAL_HANDLES {
            handles.push(table.insert(Value::Bool(true)).unwrap());
        }
        assert!(table.insert(Value::Bool(true)).is_err());
        for handle in handles {
            table.decref(handle);
        }
        assert!(table.is_empty());
        assert!(table.insert(Value::Bool(true)).is_ok());

        let mut table = EmvalTable::default();
        let chunk = Value::Bytes(vec![0; 16 * 1024 * 1024]);
        let mut held = Vec::new();
        for _ in 0..4 {
            held.push(table.insert(chunk.clone()).unwrap());
        }
        assert!(table.insert(chunk.clone()).is_err());
        for handle in held {
            table.decref(handle);
        }
        assert!(table.insert(chunk).is_ok());
    }
}
