//! Calling embind-registered functions.
//!
//! An embind function is not a wasm export. Each registration carries two table
//! indices: a generic *invoker* and the function itself. Calling it means
//! looking the invoker up in `__indirect_function_table` and passing the
//! function pointer as its first argument, with the remaining arguments encoded
//! the way emscripten's JS glue would encode them.
//!
//! Only the wire encodings the captured modules actually use are implemented.
//! An unsupported type is an explicit error rather than a silently wrong value,
//! because a plausible-but-wrong result from an oracle is worse than no result.

use anyhow::{Context, Result, anyhow, ensure};
use wasmtime::{Ref, Val};

use crate::runtime::Runtime;

/// A value crossing the embind boundary.
#[derive(Debug, Clone, PartialEq)]
pub enum Value {
    /// No value.
    Void,
    /// A `bool`, kept apart from `Int` because the caller asked a yes/no
    /// question even though the wire is the same i32.
    Bool(bool),
    /// An integer of any width the ABI marshals as one.
    Int(i64),
    /// An unsigned integer too large for `Int`.
    UInt(u64),
    /// A double.
    Double(f64),
    /// A string, already copied out of guest memory.
    Str(String),
    /// A live instance of a registered class, by pointer. Build one with
    /// `Runtime::build_vector` and release it with `Runtime::release`.
    Object(Handle),
    /// A byte vector to be marshalled into a registered `std::vector<uint8_t>`
    /// class such as `Uint8List`. The instance is built and released around the
    /// call, so callers do not manage its lifetime.
    Bytes(Vec<u8>),
    /// A string vector, marshalled into a class like `StringList`.
    StringList(Vec<String>),
    /// An integer vector, marshalled into a class like `IntList`.
    IntList(Vec<i64>),
}

/// A pointer to a live embind object, tagged with the class that owns it so it
/// can be released through the right destructor.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Handle {
    /// Address of the object in guest memory.
    pub ptr: u32,
    /// Type id of its registered class.
    pub class_type: u32,
}

impl Value {
    /// The string, if this is one.
    #[must_use]
    pub fn as_str(&self) -> Option<&str> {
        match self {
            Self::Str(value) => Some(value),
            _ => None,
        }
    }

    /// The integer, if this is one.
    #[must_use]
    pub fn as_int(&self) -> Option<i64> {
        match self {
            Self::Int(value) => Some(*value),
            Self::UInt(value) => i64::try_from(*value).ok(),
            Self::Bool(value) => Some(*value as i64),
            _ => None,
        }
    }

    /// The object handle, if this is one.
    #[must_use]
    pub fn as_handle(&self) -> Option<Handle> {
        match self {
            Self::Object(handle) => Some(*handle),
            _ => None,
        }
    }

    /// The nonnegative integer, if this value can be represented as one.
    #[must_use]
    pub fn as_uint(&self) -> Option<u64> {
        match self {
            Self::Int(value) => u64::try_from(*value).ok(),
            Self::UInt(value) => Some(*value),
            Self::Bool(value) => Some(*value as u64),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct IntegerWire {
    bytes: u8,
    signed: bool,
}

/// How a registered C++ type is passed across the boundary.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Wire {
    Void,
    /// A C++ `bool`. Travels as i32 like the integers, but is kept distinct so
    /// a predicate comes back as `Value::Bool` rather than as 0/1 — the caller
    /// asked a yes/no question and should get one.
    Bool,
    Integer(IntegerWire),
    F32,
    F64,
    /// A pointer to `{ u32 length; u8 bytes[length] }` in linear memory.
    StdString,
    /// An instance of a registered class, passed as a raw pointer.
    Class(u32),
    /// An `emscripten::val` handle. The integer is meaningless on its own; the
    /// value it names lives in the host's emval table.
    Emval,
}

/// Marker: the type name is not a primitive. The caller decides whether it
/// names a registered class instead.
struct UnknownWire;

/// Classifies a primitive type by the name embind registered for it. The name
/// is the only description available — embind records no structural detail —
/// but it comes from the module itself rather than from a guess.
fn primitive_wire(type_name: &str) -> Result<Wire, UnknownWire> {
    Ok(match type_name {
        "void" => Wire::Void,
        "bool" => Wire::Bool,
        "char" | "signed char" => Wire::Integer(IntegerWire {
            bytes: 1,
            signed: true,
        }),
        "unsigned char" => Wire::Integer(IntegerWire {
            bytes: 1,
            signed: false,
        }),
        "short" => Wire::Integer(IntegerWire {
            bytes: 2,
            signed: true,
        }),
        "unsigned short" => Wire::Integer(IntegerWire {
            bytes: 2,
            signed: false,
        }),
        "int" | "long" => Wire::Integer(IntegerWire {
            bytes: 4,
            signed: true,
        }),
        "unsigned int" | "unsigned long" => Wire::Integer(IntegerWire {
            bytes: 4,
            signed: false,
        }),
        "int64_t" | "long long" => Wire::Integer(IntegerWire {
            bytes: 8,
            signed: true,
        }),
        "uint64_t" | "unsigned long long" => Wire::Integer(IntegerWire {
            bytes: 8,
            signed: false,
        }),
        "float" => Wire::F32,
        "double" => Wire::F64,
        "std::string" | "std::basic_string<unsigned char>" => Wire::StdString,
        "emscripten::val" => Wire::Emval,
        _ => return Err(UnknownWire),
    })
}

fn encode_integer(value: &Value, wire: IntegerWire) -> Result<Val> {
    let bits = u32::from(wire.bytes) * 8;
    if wire.signed {
        let value = match value {
            Value::Bool(value) => *value as i64,
            Value::Int(value) => *value,
            Value::UInt(value) => i64::try_from(*value)
                .map_err(|_| anyhow!("{value} is outside the signed {bits}-bit range"))?,
            other => return Err(anyhow!("cannot pass {other:?} as a signed integer")),
        };
        let (minimum, maximum) = if bits == 64 {
            (i64::MIN, i64::MAX)
        } else {
            (-(1_i64 << (bits - 1)), (1_i64 << (bits - 1)) - 1)
        };
        ensure!(
            (minimum..=maximum).contains(&value),
            "{value} is outside the signed {bits}-bit range"
        );
        return Ok(if wire.bytes <= 4 {
            Val::I32(value as i32)
        } else {
            Val::I64(value)
        });
    }

    let value = match value {
        Value::Bool(value) => *value as u64,
        Value::Int(value) => u64::try_from(*value)
            .map_err(|_| anyhow!("{value} is outside the unsigned {bits}-bit range"))?,
        Value::UInt(value) => *value,
        other => return Err(anyhow!("cannot pass {other:?} as an unsigned integer")),
    };
    let maximum = if bits == 64 {
        u64::MAX
    } else {
        (1_u64 << bits) - 1
    };
    ensure!(
        value <= maximum,
        "{value} is outside the unsigned {bits}-bit range"
    );
    Ok(if wire.bytes <= 4 {
        Val::I32(value as u32 as i32)
    } else {
        Val::I64(value as i64)
    })
}

fn decode_integer(wire: IntegerWire, value: Option<&Val>) -> Result<Value> {
    let bits = u32::from(wire.bytes) * 8;
    let raw = match (wire.bytes, value) {
        (1..=4, Some(Val::I32(value))) => u64::from(*value as u32),
        (8, Some(Val::I64(value))) => *value as u64,
        (_, value) => return Err(anyhow!("unexpected integer result: {value:?}")),
    };
    let raw = if bits == 64 {
        raw
    } else {
        raw & ((1_u64 << bits) - 1)
    };
    if wire.signed {
        let shift = 64 - bits;
        Ok(Value::Int(((raw << shift) as i64) >> shift))
    } else if let Ok(value) = i64::try_from(raw) {
        Ok(Value::Int(value))
    } else {
        Ok(Value::UInt(raw))
    }
}

impl Runtime {
    /// Calls a registered embind free function by name.
    pub fn call_embind(&mut self, name: &str, args: &[Value]) -> Result<Value> {
        let function = self
            .embind_function(name)
            .ok_or_else(|| anyhow!("no embind function `{name}`"))?;

        let (result_type, param_types) = function
            .arg_types
            .split_first()
            .ok_or_else(|| anyhow!("`{name}` registered no argument types"))?;
        let (result_type, param_types) = (*result_type, param_types.to_vec());
        let (invoker, target) = (function.invoker, function.function);

        if args.len() != param_types.len() {
            return Err(anyhow!(
                "`{name}` takes {} argument(s), got {}",
                param_types.len(),
                args.len()
            ));
        }

        let outcome = self.with_owned(|this, owned| {
            // The invoker's first parameter is the function pointer itself.
            let mut call_args = vec![Val::I32(target as i32)];
            for (arg, type_id) in args.iter().zip(&param_types) {
                let wire = this.wire_for(*type_id).with_context(|| {
                    format!("argument of `{name}` (type `{}`)", this.type_name(*type_id))
                })?;
                call_args.push(this.encode(arg, wire, owned)?);
            }

            let result_wire = this
                .wire_for(result_type)
                .with_context(|| format!("return type of `{name}`"))?;
            this.invoke(invoker, &call_args, result_wire)
        });

        outcome.with_context(|| format!("calling embind function `{name}`"))
    }

    /// Resolves a registered type id to its wire encoding, falling back to a
    /// class instance when the name is not a primitive.
    fn wire_for(&self, type_id: u32) -> Result<Wire> {
        let type_name = self.type_name(type_id);
        if let Some(integer) = self.integer_type(type_id) {
            ensure!(
                matches!(integer.bytes, 1 | 2 | 4 | 8),
                "type `{type_name}` registered unsupported integer width {}",
                integer.bytes
            );
            return Ok(Wire::Integer(IntegerWire {
                bytes: integer.bytes as u8,
                signed: integer.signed,
            }));
        }
        if let Ok(wire) = primitive_wire(&type_name) {
            return Ok(wire);
        }
        if self.embind_class(type_id).is_some() {
            return Ok(Wire::Class(type_id));
        }
        Err(anyhow!(
            "type `{type_name}` is neither a primitive nor a registered class"
        ))
    }

    /// Encodes one argument, recording anything that must be released after.
    fn encode(&mut self, value: &Value, wire: Wire, owned: &mut Owned) -> Result<Val> {
        Ok(match (value, wire) {
            (Value::Bool(value), Wire::Bool) => Val::I32(*value as i32),
            (Value::Int(value), Wire::Bool) => Val::I32(i32::from(*value != 0)),
            (value, Wire::Integer(integer)) => encode_integer(value, integer)?,
            (Value::Double(value), Wire::F64) => Val::F64(value.to_bits()),
            (Value::Double(value), Wire::F32) => Val::F32((*value as f32).to_bits()),
            (Value::Int(value), Wire::F64) => Val::F64((*value as f64).to_bits()),
            (Value::Str(text), Wire::StdString) => {
                let ptr = self.write_std_string(text)?;
                owned.allocations.push(ptr);
                Val::I32(ptr as i32)
            }
            (Value::Object(handle), Wire::Class(class_type)) => {
                if handle.class_type != class_type {
                    return Err(anyhow!(
                        "handle belongs to `{}`, not `{}`",
                        self.type_name(handle.class_type),
                        self.type_name(class_type)
                    ));
                }
                // Caller-owned: do not release it here.
                Val::I32(handle.ptr as i32)
            }
            (Value::Bytes(bytes), Wire::Class(class_type)) => {
                let values: Vec<i64> = bytes.iter().map(|byte| *byte as i64).collect();
                let handle = self.build_vector(class_type, &values, &[])?;
                owned.objects.push(handle);
                Val::I32(handle.ptr as i32)
            }
            (Value::IntList(values), Wire::Class(class_type)) => {
                let handle = self.build_vector(class_type, values, &[])?;
                owned.objects.push(handle);
                Val::I32(handle.ptr as i32)
            }
            (Value::StringList(items), Wire::Class(class_type)) => {
                let handle = self.build_vector(class_type, &[], items)?;
                owned.objects.push(handle);
                Val::I32(handle.ptr as i32)
            }
            (value, wire) => {
                return Err(anyhow!("cannot pass {value:?} as {wire:?}"));
            }
        })
    }

    fn release_owned(&mut self, owned: Owned) {
        for handle in owned.objects {
            let _ = self.release(handle);
        }
        for ptr in owned.allocations {
            let _ = self.free(ptr);
        }
    }

    fn with_owned<T>(
        &mut self,
        operation: impl FnOnce(&mut Self, &mut Owned) -> Result<T>,
    ) -> Result<T> {
        let mut owned = Owned::default();
        let result = operation(self, &mut owned);
        self.release_owned(owned);
        result
    }

    /// Looks the invoker up in the function table and calls it.
    fn invoke(&mut self, invoker: u32, args: &[Val], result: Wire) -> Result<Value> {
        let callee = self.table_function(invoker)?;

        let mut results = match result {
            Wire::Void => vec![],
            Wire::Bool | Wire::StdString | Wire::Class(_) | Wire::Emval => {
                vec![Val::I32(0)]
            }
            Wire::Integer(integer) if integer.bytes <= 4 => vec![Val::I32(0)],
            Wire::Integer(_) => vec![Val::I64(0)],
            Wire::F32 => vec![Val::F32(0)],
            Wire::F64 => vec![Val::F64(0)],
        };

        self.call_func(callee, args, &mut results)?;

        Ok(match (result, results.first()) {
            (Wire::Void, _) => Value::Void,
            (Wire::Bool, Some(Val::I32(value))) => Value::Bool(*value != 0),
            (Wire::Integer(integer), value) => decode_integer(integer, value)?,
            (Wire::F32, Some(Val::F32(bits))) => Value::Double(f32::from_bits(*bits) as f64),
            (Wire::F64, Some(Val::F64(bits))) => Value::Double(f64::from_bits(*bits)),
            (Wire::Emval, Some(Val::I32(handle))) => {
                let handle = *handle as u32;
                let value = self
                    .emval_value(handle)
                    .ok_or_else(|| anyhow!("emval handle {handle} names no value"))?;
                // The guest handed ownership over with the handle; releasing it
                // here keeps the table from growing across a loop of calls.
                self.emval_release(handle);
                value
            }
            (Wire::Class(class_type), Some(Val::I32(ptr))) => Value::Object(Handle {
                ptr: *ptr as u32,
                class_type,
            }),
            (Wire::StdString, Some(Val::I32(ptr))) => {
                let text = self.read_std_string(*ptr as u32)?;
                // embind's glue frees the returned string after copying it out.
                let _ = self.free(*ptr as u32);
                Value::Str(text)
            }
            (wire, value) => return Err(anyhow!("unexpected {wire:?} result: {value:?}")),
        })
    }

    /// Writes a `std::string` in the layout embind expects: a 32-bit length
    /// followed by the bytes.
    fn write_std_string(&mut self, text: &str) -> Result<u32> {
        let bytes = text.as_bytes();
        let ptr = self.malloc(4 + bytes.len() as u32 + 1)?;

        let mut buffer = Vec::with_capacity(4 + bytes.len() + 1);
        buffer.extend_from_slice(&(bytes.len() as u32).to_le_bytes());
        buffer.extend_from_slice(bytes);
        buffer.push(0);

        self.write_bytes_at(ptr, &buffer)?;
        Ok(ptr)
    }

    fn read_std_string(&self, ptr: u32) -> Result<String> {
        const MAX: u32 = 16 * 1024 * 1024;

        if ptr == 0 {
            return Ok(String::new());
        }
        let length = self.state().read_u32(ptr)?;
        if length > MAX {
            return Err(anyhow!("returned string claims {length} bytes"));
        }
        let bytes = self.state().read(ptr + 4, length)?;
        Ok(String::from_utf8_lossy(&bytes).into_owned())
    }

    /// Resolves a function-table index.
    fn table_function(&mut self, index: u32) -> Result<wasmtime::Func> {
        let table = self.function_table()?;
        match self.table_get(table, index as u64) {
            Some(Ref::Func(Some(func))) => Ok(func),
            _ => Err(anyhow!("function table entry {index} is not callable")),
        }
    }
}

/// Temporaries created for one call, released once it returns.
#[derive(Debug, Default)]
struct Owned {
    /// Raw `malloc` allocations, e.g. marshalled strings.
    allocations: Vec<u32>,
    /// Class instances built for this call.
    objects: Vec<Handle>,
}

impl Runtime {
    /// Builds a registered vector class (`Uint8List`, `IntList`, `StringList`)
    /// and fills it.
    ///
    /// embind registers these through `register_vector`, which yields a default
    /// constructor plus `push_back`. There is no bulk path: the vector is filled
    /// one element at a time, exactly as the JS glue does.
    ///
    /// Exactly one of `numbers` or `strings` is used, chosen by `push_back`'s
    /// registered element type — passing the wrong one is an error rather than a
    /// silent coercion.
    pub fn build_vector(
        &mut self,
        class_type: u32,
        numbers: &[i64],
        strings: &[String],
    ) -> Result<Handle> {
        let class = self
            .embind_class(class_type)
            .ok_or_else(|| anyhow!("type {class_type} is not a registered class"))?;
        let class_name = class.name.clone();

        let constructor = class
            .constructors
            .iter()
            .find(|constructor| constructor.arg_types.len() == 1)
            .ok_or_else(|| anyhow!("`{class_name}` has no default constructor"))?
            .clone();

        let push_back = class
            .methods
            .iter()
            .find(|method| method.name == "push_back")
            .ok_or_else(|| anyhow!("`{class_name}` has no push_back"))?
            .clone();

        // push_back is registered as (return, this, element); the element type
        // is what decides how the values are encoded.
        let element_type = *push_back
            .arg_types
            .get(2)
            .ok_or_else(|| anyhow!("`{class_name}::push_back` has no element type"))?;
        let element_wire = self.wire_for(element_type)?;

        let invoker = self.table_function(constructor.invoker)?;
        let mut results = vec![Val::I32(0)];
        self.call_func(
            invoker,
            &[Val::I32(constructor.constructor as i32)],
            &mut results,
        )
        .with_context(|| format!("constructing `{class_name}`"))?;

        let Some(Val::I32(ptr)) = results.first() else {
            return Err(anyhow!("`{class_name}` constructor returned no pointer"));
        };
        let handle = Handle {
            ptr: *ptr as u32,
            class_type,
        };

        let outcome = self.fill_vector(
            handle,
            &class_name,
            &push_back,
            element_wire,
            numbers,
            strings,
        );
        if outcome.is_err() {
            // Do not leak a partially built vector.
            let _ = self.release(handle);
        }
        outcome?;

        Ok(handle)
    }

    fn fill_vector(
        &mut self,
        handle: Handle,
        class_name: &str,
        push_back: &crate::embind::EmbindMethod,
        element_wire: Wire,
        numbers: &[i64],
        strings: &[String],
    ) -> Result<()> {
        let values: Vec<Value> = match element_wire {
            Wire::StdString => {
                if !numbers.is_empty() {
                    return Err(anyhow!("`{class_name}` holds strings, got numbers"));
                }
                strings.iter().cloned().map(Value::Str).collect()
            }
            Wire::Integer(_) => {
                if !strings.is_empty() {
                    return Err(anyhow!("`{class_name}` holds numbers, got strings"));
                }
                numbers.iter().copied().map(Value::Int).collect()
            }
            other => {
                return Err(anyhow!(
                    "`{class_name}` has element encoding {other:?}, which is not supported"
                ));
            }
        };

        for value in &values {
            self.with_owned(|this, owned| {
                let encoded = this.encode(value, element_wire, owned)?;

                // A method invoker takes (context, this, args...).
                let invoker = this.table_function(push_back.invoker)?;
                let args = [
                    Val::I32(push_back.context as i32),
                    Val::I32(handle.ptr as i32),
                    encoded,
                ];
                this.call_func(invoker, &args, &mut [])
                    .with_context(|| format!("`{class_name}::push_back`"))
            })?;
        }

        Ok(())
    }

    /// Reads back the size of a registered vector, which is how a test confirms
    /// the module saw what was pushed rather than trusting the push calls.
    pub fn vector_len(&mut self, handle: Handle) -> Result<i64> {
        let class = self
            .embind_class(handle.class_type)
            .ok_or_else(|| anyhow!("unknown class {}", handle.class_type))?;
        let size = class
            .methods
            .iter()
            .find(|method| method.name == "size")
            .ok_or_else(|| anyhow!("`{}` has no size method", class.name))?
            .clone();

        let invoker = self.table_function(size.invoker)?;
        let mut results = vec![Val::I32(0)];
        self.call_func(
            invoker,
            &[Val::I32(size.context as i32), Val::I32(handle.ptr as i32)],
            &mut results,
        )?;

        match results.first() {
            Some(Val::I32(len)) => Ok(*len as i64),
            other => Err(anyhow!("size returned {other:?}")),
        }
    }

    /// Destroys an embind instance through its registered destructor.
    pub fn release(&mut self, handle: Handle) -> Result<()> {
        let class = self
            .embind_class(handle.class_type)
            .ok_or_else(|| anyhow!("unknown class {}", handle.class_type))?;
        let destructor = class.destructor;
        if destructor == 0 {
            // No registered destructor; the allocation is the module's problem.
            return Ok(());
        }

        let func = self.table_function(destructor)?;
        self.call_func(func, &[Val::I32(handle.ptr as i32)], &mut [])
            .context("destroying embind instance")
    }

    /// Finds a registered class by the name embind gave it.
    pub fn class_type_by_name(&mut self, name: &str) -> Option<u32> {
        self.embind()
            .classes
            .values()
            .find(|class| class.name == name)
            .map(|class| class.type_id)
    }
}

impl Runtime {
    /// Calls a method on a registered class instance.
    ///
    /// A method invoker takes `(context, this, args...)`, where `context` is the
    /// member function pointer the registration recorded — unlike a free
    /// function, whose invoker takes the function pointer directly.
    pub fn call_method(&mut self, handle: Handle, method: &str, args: &[Value]) -> Result<Value> {
        let class = self
            .embind_class(handle.class_type)
            .ok_or_else(|| anyhow!("type {} is not a registered class", handle.class_type))?;
        let class_name = class.name.clone();

        let declared = class
            .methods
            .iter()
            .find(|candidate| candidate.name == method)
            .cloned()
            .ok_or_else(|| anyhow!("`{class_name}` has no method `{method}`"))?;

        // arg_types is (return, this, params...): the instance occupies a slot,
        // so the caller's arguments start at index 2.
        let (result_type, rest) = declared
            .arg_types
            .split_first()
            .ok_or_else(|| anyhow!("`{class_name}::{method}` registered no types"))?;
        let param_types: Vec<u32> = rest.iter().skip(1).copied().collect();

        if args.len() != param_types.len() {
            return Err(anyhow!(
                "`{class_name}::{method}` takes {} argument(s), got {}",
                param_types.len(),
                args.len()
            ));
        }

        let outcome = self.with_owned(|this, owned| {
            let mut call_args = vec![
                Val::I32(declared.context as i32),
                Val::I32(handle.ptr as i32),
            ];
            for (arg, type_id) in args.iter().zip(&param_types) {
                let wire = this.wire_for(*type_id).with_context(|| {
                    format!(
                        "argument of `{class_name}::{method}` (type `{}`)",
                        this.type_name(*type_id)
                    )
                })?;
                call_args.push(this.encode(arg, wire, owned)?);
            }

            let result_wire = this
                .wire_for(*result_type)
                .with_context(|| format!("return type of `{class_name}::{method}`"))?;
            this.invoke(declared.invoker, &call_args, result_wire)
        });

        outcome.with_context(|| format!("calling `{class_name}::{method}`"))
    }

    /// Reads a registered vector back element by element.
    ///
    /// This is the direction that was missing: a vector could be built and
    /// handed to the module, but whatever the module put in one was unreadable
    /// until `get` — which returns `emscripten::val` — could be decoded.
    pub fn read_vector(&mut self, handle: Handle) -> Result<Vec<Value>> {
        let len = self.vector_len(handle)?;
        (0..len)
            .map(|index| self.call_method(handle, "get", &[Value::Int(index)]))
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unsigned_integer_wires_preserve_values_and_reject_overflow() {
        let u32_wire = IntegerWire {
            bytes: 4,
            signed: false,
        };
        assert!(matches!(
            encode_integer(&Value::Int(i64::from(u32::MAX)), u32_wire).unwrap(),
            Val::I32(-1)
        ));
        assert_eq!(
            decode_integer(u32_wire, Some(&Val::I32(-1))).unwrap(),
            Value::Int(i64::from(u32::MAX))
        );
        assert!(encode_integer(&Value::Int(-1), u32_wire).is_err());
        assert!(encode_integer(&Value::UInt(u64::from(u32::MAX) + 1), u32_wire).is_err());

        let u64_wire = IntegerWire {
            bytes: 8,
            signed: false,
        };
        assert_eq!(
            decode_integer(u64_wire, Some(&Val::I64(-1))).unwrap(),
            Value::UInt(u64::MAX)
        );
    }

    #[test]
    fn narrow_signed_results_are_sign_extended() {
        let wire = IntegerWire {
            bytes: 1,
            signed: true,
        };
        assert_eq!(
            decode_integer(wire, Some(&Val::I32(0xff))).unwrap(),
            Value::Int(-1)
        );
        assert!(encode_integer(&Value::Int(128), wire).is_err());
    }
}
