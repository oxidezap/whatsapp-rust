//! Recovers an emscripten module's embind API.
//!
//! A module built with embind exports almost nothing useful: its real surface
//! is registered at runtime, when the C++ static constructors call back into
//! `_embind_register_*`. Those calls are normally serviced by emscripten's JS
//! glue, which is not part of the capture. Implementing them here recovers the
//! function names, class names, method names and type signatures that the
//! exports section does not contain.
//!
//! Argument layouts follow emscripten's `embind.cpp`. Where a registration
//! passes an argument-type array, its first element is the *return* type and
//! the rest are parameters.

use std::collections::BTreeMap;

use anyhow::Result;
use wasmtime::error::Context as _;
use wasmtime::{Caller, Linker, Module, Store, Val};

use crate::integer::IntegerType;
use crate::state::HostState;

/// A free function the module registered with embind.
#[derive(Debug, Clone)]
pub struct EmbindFunction {
    /// The name JavaScript would call it by, and the only name it has: none of
    /// this is in the module's exports.
    pub name: String,
    /// Type ids: index 0 is the return type, the rest are parameters.
    pub arg_types: Vec<u32>,
    /// Table index of the generic invoker emscripten would call.
    pub invoker: u32,
    /// Table index of the function itself.
    pub function: u32,
    /// Whether it was registered as returning a promise.
    pub is_async: bool,
}

/// A method on a registered class.
///
/// Its invoker takes `(context, this, args…)`, unlike a free function's — which
/// is the difference that made values the module *produced* unreadable until
/// `call_method` existed.
#[derive(Debug, Clone)]
pub struct EmbindMethod {
    /// Type id of the class it belongs to.
    pub class_type: u32,
    /// The method name.
    pub name: String,
    /// Type ids: index 0 is the return type, the rest are parameters.
    pub arg_types: Vec<u32>,
    /// Table index of the invoker to call.
    pub invoker: u32,
    /// First argument the invoker expects, registered alongside it.
    pub context: u32,
    /// Whether it was registered as pure virtual, in which case there is no
    /// body to reach.
    pub is_pure_virtual: bool,
    /// Whether it was registered as returning a promise.
    pub is_async: bool,
}

/// A data member a class registered through `_embind_register_class_property`.
///
/// Not a method, and the layout is not a method's either: the third argument is
/// the field's *type*, and what follows is a getter signature, getter and
/// context, then the same three for a setter. Reading that as `(argCount,
/// argTypes…)` took a signature string for an array of type ids and recorded
/// the wrong invoker, so the registry offered a callable that dispatched to
/// whatever table slot the misread produced.
#[derive(Debug, Clone)]
pub struct EmbindProperty {
    /// Type id of the class it belongs to.
    pub class_type: u32,
    /// The property name.
    pub name: String,
    /// Type id of the value the getter returns.
    pub field_type: u32,
    /// Table index of the getter's invoker.
    pub getter: u32,
    /// First argument that invoker expects.
    pub getter_context: u32,
    /// Type id the setter takes, or `None` for a read-only property — which
    /// embind registers by passing 0 for the setter's type.
    pub setter_type: Option<u32>,
    /// Table index of the setter's invoker, 0 when read-only.
    pub setter: u32,
    /// First argument that invoker expects.
    pub setter_context: u32,
}

/// A registered constructor.
#[derive(Debug, Clone)]
pub struct EmbindConstructor {
    /// Type id of the class it builds.
    pub class_type: u32,
    /// Type ids: index 0 is the return type, the rest are parameters.
    pub arg_types: Vec<u32>,
    /// Table index of the invoker to call.
    pub invoker: u32,
    /// Table index of the constructor itself.
    pub constructor: u32,
}

/// A class the module registered.
#[derive(Debug, Clone, Default)]
pub struct EmbindClass {
    /// The registered name, e.g. `Uint8List`.
    pub name: String,
    /// Its own type id.
    pub type_id: u32,
    /// Type id of its base class, or 0.
    pub base_type: u32,
    /// Table index of the destructor, needed to release an instance.
    pub destructor: u32,
    /// Its methods, in registration order.
    pub methods: Vec<EmbindMethod>,
    /// Its constructors, in registration order.
    pub constructors: Vec<EmbindConstructor>,
    /// Its data members, in registration order. See [`EmbindProperty`].
    pub properties: Vec<EmbindProperty>,
}

/// Everything the module registered, keyed so it can be resolved after the fact.
#[derive(Debug, Clone, Default)]
pub struct EmbindRegistry {
    /// Type id to type name, from the primitive and class registrations.
    pub types: BTreeMap<u32, String>,
    integer_types: BTreeMap<u32, IntegerType>,
    /// Free functions, in registration order.
    pub functions: Vec<EmbindFunction>,
    /// Classes, by type id.
    pub classes: BTreeMap<u32, EmbindClass>,
    /// Enum values and value-object fields, as (owner type, name, value).
    pub members: Vec<(u32, String, i64)>,
    /// Registered constants, as (name, value).
    pub constants: Vec<(String, i64)>,
    /// Registrations whose class had not been registered yet when they arrived.
    orphan_methods: Vec<EmbindMethod>,
    orphan_constructors: Vec<EmbindConstructor>,
    orphan_properties: Vec<EmbindProperty>,
}

impl EmbindRegistry {
    /// Whether nothing was registered at all, which is the correct answer for
    /// a module that exposes plain C exports instead.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.functions.is_empty() && self.classes.is_empty()
    }

    /// Resolves a type id to the name the module registered for it.
    pub fn type_name(&self, id: u32) -> String {
        self.types
            .get(&id)
            .cloned()
            .unwrap_or_else(|| format!("type#{id}"))
    }

    pub(crate) fn integer_type(&self, id: u32) -> Option<IntegerType> {
        self.integer_types.get(&id).copied()
    }

    /// Renders a signature the way a C++ declaration reads.
    pub fn signature(&self, arg_types: &[u32]) -> String {
        let Some((result, params)) = arg_types.split_first() else {
            return "()".to_owned();
        };
        let params = params
            .iter()
            .map(|id| self.type_name(*id))
            .collect::<Vec<_>>()
            .join(", ");
        format!("({params}) -> {}", self.type_name(*result))
    }

    /// Attaches registrations that arrived before their class did. Registration
    /// order is not guaranteed across translation units, so this runs at the end
    /// rather than at each call.
    pub fn settle(&mut self) {
        for method in std::mem::take(&mut self.orphan_methods) {
            match self.classes.get_mut(&method.class_type) {
                Some(class) => class.methods.push(method),
                None => self
                    .classes
                    .entry(method.class_type)
                    .or_insert_with(|| EmbindClass {
                        name: format!("class#{}", method.class_type),
                        type_id: method.class_type,
                        ..EmbindClass::default()
                    })
                    .methods
                    .push(method),
            }
        }
        for constructor in std::mem::take(&mut self.orphan_constructors) {
            self.classes
                .entry(constructor.class_type)
                .or_insert_with(|| EmbindClass {
                    name: format!("class#{}", constructor.class_type),
                    type_id: constructor.class_type,
                    ..EmbindClass::default()
                })
                .constructors
                .push(constructor);
        }
        for property in std::mem::take(&mut self.orphan_properties) {
            self.classes
                .entry(property.class_type)
                .or_insert_with(|| EmbindClass {
                    name: format!("class#{}", property.class_type),
                    type_id: property.class_type,
                    ..EmbindClass::default()
                })
                .properties
                .push(property);
        }

        self.functions.sort_by(|a, b| a.name.cmp(&b.name));
        for class in self.classes.values_mut() {
            class.methods.sort_by(|a, b| a.name.cmp(&b.name));
            class.properties.sort_by(|a, b| a.name.cmp(&b.name));
            if let Some(name) = self.types.get(&class.type_id) {
                class.name = name.clone();
            }
        }
    }

    /// Total methods across every registered class.
    #[must_use]
    pub fn method_count(&self) -> usize {
        self.classes.values().map(|class| class.methods.len()).sum()
    }

    /// Total data members across every registered class.
    #[must_use]
    pub fn property_count(&self) -> usize {
        self.classes
            .values()
            .map(|class| class.properties.len())
            .sum()
    }
}

/// Reads `count` type ids from an array of 32-bit ids.
fn read_type_ids(state: &HostState, ptr: u32, count: u32) -> Result<Vec<u32>> {
    anyhow::ensure!(count <= 64, "embind type count exceeds 64: {count}");
    let len = count.checked_mul(4).context("type array length overflow")?;
    let bytes = state.read(ptr, len)?;
    Ok(bytes
        .chunks_exact(4)
        .map(|word| u32::from_le_bytes(word.try_into().expect("four-byte type id")))
        .collect())
}

/// Where the interesting operands sit in a registration's argument list.
///
/// Emscripten changes these layouts between releases — `_embind_register_bigint`
/// takes five arguments in one captured module and seven in another — so the
/// handler is built from the signature the module itself declares, and an index
/// past the end is skipped rather than read as garbage.
#[derive(Debug, Clone, Copy)]
enum Layout {
    /// A named type: the id is argument 0 and the name is at `name_at`.
    Type {
        name_at: usize,
    },
    Integer,
    /// A free function.
    Function,
    /// A class declaration.
    Class,
    /// A method on a class. `is_static` picks between the two spellings, which
    /// agree up to the last two arguments: an instance method carries
    /// `(isPureVirtual, isAsync)` and a static one carries `(isAsync)` alone.
    ClassFunction {
        /// Whether this is `_embind_register_class_class_function`.
        is_static: bool,
    },
    /// A data member on a class. See [`EmbindProperty`].
    ClassProperty,
    /// A constructor for a class.
    ClassConstructor,
    /// An enum type, then its values.
    EnumValue,
    /// A named constant.
    Constant,
}

/// Maps a registration import to its layout. Anything not listed is left to the
/// generic stub, which records the call without interpreting it.
fn layout_of(name: &str) -> Option<Layout> {
    Some(match name {
        // (type, name, ...)
        "_embind_register_void"
        | "_embind_register_bool"
        | "_embind_register_float"
        | "_embind_register_std_string"
        | "_embind_register_emval"
        | "_embind_register_enum"
        | "_embind_register_value_object"
        | "_embind_register_value_array"
        | "_embind_register_smart_ptr"
        | "_embind_register_user_type" => Layout::Type { name_at: 1 },

        "_embind_register_integer" | "_embind_register_bigint" => Layout::Integer,

        // (type, <something>, name, ...)
        "_embind_register_std_wstring" | "_embind_register_memory_view" => {
            Layout::Type { name_at: 2 }
        }

        "_embind_register_function" => Layout::Function,
        "_embind_register_class" => Layout::Class,
        "_embind_register_class_function" => Layout::ClassFunction { is_static: false },
        "_embind_register_class_class_function" => Layout::ClassFunction { is_static: true },
        "_embind_register_class_property" => Layout::ClassProperty,
        "_embind_register_class_constructor" => Layout::ClassConstructor,
        "_embind_register_enum_value" | "_embind_register_value_object_field" => Layout::EnumValue,
        "_embind_register_constant" => Layout::Constant,
        _ => return None,
    })
}

/// Reads argument `index` as a `u32`, or `None` when the registration is
/// shorter than this layout expects.
fn arg(args: &[Val], index: usize) -> Option<u32> {
    match args.get(index)? {
        Val::I32(value) => Some(*value as u32),
        Val::I64(value) => Some(*value as u32),
        _ => None,
    }
}

fn signed_arg(args: &[Val], index: usize) -> Option<i64> {
    match args.get(index)? {
        Val::I32(value) => Some(i64::from(*value)),
        Val::I64(value) => Some(*value),
        _ => None,
    }
}

fn integer_minimum(args: &[Val]) -> Option<i64> {
    if args.len() >= 7 {
        let low = u64::from(arg(args, 3)?);
        let high = u64::from(arg(args, 4)?);
        Some(((high << 32) | low) as i64)
    } else {
        signed_arg(args, 3)
    }
}

/// Applies one registration to the registry.
fn apply(caller: &mut Caller<'_, HostState>, layout: Layout, args: &[Val]) -> Result<()> {
    match layout {
        Layout::Type { name_at } => {
            let (Some(id), Some(name_ptr)) = (arg(args, 0), arg(args, name_at)) else {
                return Ok(());
            };
            let name = caller.data().read_cstr(name_ptr).unwrap_or_default();
            if !name.is_empty() {
                caller.data_mut().embind.types.insert(id, name);
            }
        }

        Layout::Integer => {
            // (type, name, size, min, max). The minimum carries signedness;
            // size decides whether the ABI value travels as i32 or i64.
            let (Some(id), Some(name_ptr), Some(bytes), Some(minimum)) = (
                arg(args, 0),
                arg(args, 1),
                arg(args, 2),
                integer_minimum(args),
            ) else {
                return Ok(());
            };
            let name = caller.data().read_cstr(name_ptr).unwrap_or_default();
            if !name.is_empty() {
                let state = caller.data_mut();
                let integer = IntegerType::new(bytes, minimum < 0)?;
                state.embind.types.insert(id, name);
                state.embind.integer_types.insert(id, integer);
            }
        }

        Layout::Function => {
            // (name, argCount, argTypes, signature, invoker, function, isAsync)
            let (Some(name_ptr), Some(count), Some(types_ptr)) =
                (arg(args, 0), arg(args, 1), arg(args, 2))
            else {
                return Ok(());
            };
            // A registration without invoker and target operands cannot
            // dispatch: publishing it with table slot 0 lets a later call run
            // an unrelated function and report a plausible result.
            let (Some(invoker), Some(target)) = (arg(args, 4), arg(args, 5)) else {
                caller.data().log(format!(
                    "_embind_register_function: registration of arg-set {count} lacks invoker/target"
                ));
                return Ok(());
            };
            if invoker == 0 || target == 0 {
                caller.data().log(format!(
                    "_embind_register_function: registration of arg-set {count} has a null invoker/target"
                ));
                return Ok(());
            }
            let state = caller.data();
            let name = state.read_cstr(name_ptr).unwrap_or_default();
            let arg_types = read_type_ids(state, types_ptr, count)?;

            caller.data_mut().embind.functions.push(EmbindFunction {
                name,
                arg_types,
                invoker,
                function: target,
                is_async: arg(args, 6).unwrap_or(0) != 0,
            });
        }

        Layout::Class => {
            // (classType, pointerType, constPointerType, baseType,
            //  getActualTypeSignature, getActualType, upcastSignature, upcast,
            //  downcastSignature, downcast, name, destructorSignature, destructor)
            let (Some(class_type), Some(name_ptr)) = (arg(args, 0), arg(args, 10)) else {
                return Ok(());
            };
            let name = caller.data().read_cstr(name_ptr).unwrap_or_default();
            let base_type = arg(args, 3).unwrap_or(0);
            let destructor = arg(args, 12).unwrap_or(0);

            let state = caller.data_mut();
            state.embind.types.insert(class_type, name.clone());
            let class = state.embind.classes.entry(class_type).or_default();
            class.name = name;
            class.type_id = class_type;
            class.base_type = base_type;
            class.destructor = destructor;
        }

        Layout::ClassFunction { is_static } => {
            // (classType, methodName, argCount, argTypes, invokerSignature,
            //  invoker, context, isPureVirtual, isAsync)
            //
            // A static one is the same list without `isPureVirtual`, so the
            // last argument is `isAsync` and there is no pure-virtual flag to
            // read. Taking argument 7 for it either way made every async static
            // method look like one with no body to reach.
            let (Some(class_type), Some(name_ptr), Some(count), Some(types_ptr)) =
                (arg(args, 0), arg(args, 1), arg(args, 2), arg(args, 3))
            else {
                return Ok(());
            };
            let state = caller.data();
            let name = state.read_cstr(name_ptr).unwrap_or_default();
            let arg_types = read_type_ids(state, types_ptr, count)?;

            caller.data_mut().embind.orphan_methods.push(EmbindMethod {
                class_type,
                name,
                arg_types,
                invoker: arg(args, 5).unwrap_or(0),
                context: arg(args, 6).unwrap_or(0),
                is_pure_virtual: !is_static && arg(args, 7).unwrap_or(0) != 0,
                is_async: arg(args, if is_static { 7 } else { 8 }).unwrap_or(0) != 0,
            });
        }

        Layout::ClassProperty => {
            // (classType, fieldName, getterReturnType, getterSignature, getter,
            //  getterContext, setterArgumentType, setterSignature, setter,
            //  setterContext)
            let (Some(class_type), Some(name_ptr), Some(field_type)) =
                (arg(args, 0), arg(args, 1), arg(args, 2))
            else {
                return Ok(());
            };
            let name = caller.data().read_cstr(name_ptr).unwrap_or_default();
            // embind registers a read-only property by passing 0 for the
            // setter's type — there is no setter to record for one.
            let setter_type = arg(args, 6).filter(|id| *id != 0);

            caller
                .data_mut()
                .embind
                .orphan_properties
                .push(EmbindProperty {
                    class_type,
                    name,
                    field_type,
                    getter: arg(args, 4).unwrap_or(0),
                    getter_context: arg(args, 5).unwrap_or(0),
                    setter_type,
                    setter: setter_type.and(arg(args, 8)).unwrap_or(0),
                    setter_context: setter_type.and(arg(args, 9)).unwrap_or(0),
                });
        }

        Layout::ClassConstructor => {
            // (classType, argCount, argTypes, invokerSignature, invoker, constructor)
            let (Some(class_type), Some(count), Some(types_ptr)) =
                (arg(args, 0), arg(args, 1), arg(args, 2))
            else {
                return Ok(());
            };
            let arg_types = read_type_ids(caller.data(), types_ptr, count)?;

            caller
                .data_mut()
                .embind
                .orphan_constructors
                .push(EmbindConstructor {
                    class_type,
                    arg_types,
                    invoker: arg(args, 4).unwrap_or(0),
                    constructor: arg(args, 5).unwrap_or(0),
                });
        }

        Layout::EnumValue => {
            // (ownerType, name, value) — recorded so an enum's members can be
            // read back even though they are not callable.
            let (Some(owner), Some(name_ptr)) = (arg(args, 0), arg(args, 1)) else {
                return Ok(());
            };
            let name = caller.data().read_cstr(name_ptr).unwrap_or_default();
            let value = arg(args, 2).unwrap_or(0);
            caller
                .data_mut()
                .embind
                .members
                .push((owner, name, value as i64));
        }

        Layout::Constant => {
            // (name, type, value)
            let Some(name_ptr) = arg(args, 0) else {
                return Ok(());
            };
            let name = caller.data().read_cstr(name_ptr).unwrap_or_default();
            let value = arg(args, 2).unwrap_or(0);
            caller
                .data_mut()
                .embind
                .constants
                .push((name, value as i64));
        }
    }
    Ok(())
}

/// Defines every `_embind_register_*` import the module declares, deriving each
/// handler from that module's own signature for it.
pub fn define(
    store: &mut Store<HostState>,
    linker: &mut Linker<HostState>,
    module: &Module,
) -> Result<usize> {
    let mut defined = 0;

    for import in module.imports() {
        let Some(layout) = layout_of(import.name()) else {
            continue;
        };
        let wasmtime::ExternType::Func(ty) = import.ty() else {
            continue;
        };

        let func =
            crate::host::host_func(&mut *store, ty.clone(), move |caller, params, _results| {
                apply(caller, layout, params).map_err(wasmtime::Error::from_anyhow)
            });

        linker
            .define(&*store, import.module(), import.name(), func)
            .with_context(|| format!("defining {}", import.name()))?;
        defined += 1;
    }

    Ok(defined)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn type_arrays_are_never_truncated() {
        let state = HostState::default();
        assert!(read_type_ids(&state, 0, 65).is_err());
        assert!(read_type_ids(&state, u32::MAX - 1, 1).is_err());
        assert!(read_type_ids(&state, 0, 1).is_err());
    }

    #[test]
    fn legalized_bigint_bounds_preserve_signedness() {
        let signed = [
            Val::I32(1),
            Val::I32(2),
            Val::I32(8),
            Val::I32(0),
            Val::I32(i32::MIN),
            Val::I32(-1),
            Val::I32(i32::MAX),
        ];
        assert_eq!(integer_minimum(&signed), Some(i64::MIN));

        let unsigned = [
            Val::I32(1),
            Val::I32(2),
            Val::I32(8),
            Val::I32(0),
            Val::I32(0),
            Val::I32(-1),
            Val::I32(-1),
        ];
        assert_eq!(integer_minimum(&unsigned), Some(0));
        assert_eq!(
            integer_minimum(&[
                Val::I32(1),
                Val::I32(2),
                Val::I32(8),
                Val::I64(i64::MIN),
                Val::I64(i64::MAX),
            ]),
            Some(i64::MIN)
        );
    }

    #[test]
    fn registrations_without_targets_are_not_callable() {
        use wasm_encoder::{
            CodeSection, ConstExpr, DataSection, EntityType, ExportKind, ExportSection, Function,
            FunctionSection, ImportSection, MemorySection, MemoryType, Module, TypeSection,
            ValType,
        };
        fn registration_module(params: &[ValType], args: &[i32]) -> Vec<u8> {
            let mut types = TypeSection::new();
            types.ty().function(params.iter().copied(), []);
            types.ty().function([], []);
            let mut imports = ImportSection::new();
            imports.import("env", "_embind_register_function", EntityType::Function(0));
            let mut functions = FunctionSection::new();
            functions.function(1);
            let mut memories = MemorySection::new();
            memories.memory(MemoryType {
                minimum: 1,
                maximum: None,
                memory64: false,
                shared: false,
                page_size_log2: None,
            });
            let mut exports = ExportSection::new();
            exports.export("memory", ExportKind::Memory, 0);
            exports.export("reg", ExportKind::Func, 1);
            let mut body = Function::new([]);
            body.instructions().i32_const(64);
            for arg in args {
                body.instructions().i32_const(*arg);
            }
            body.instructions().call(0).end();
            let mut code = CodeSection::new();
            code.function(&body);
            let mut data = DataSection::new();
            data.active(0, &ConstExpr::i32_const(64), [b'f', 0]);
            let mut module = Module::new();
            module
                .section(&types)
                .section(&imports)
                .section(&functions)
                .section(&memories)
                .section(&exports)
                .section(&code)
                .section(&data);
            module.finish()
        }
        // A shortened signature omits the invoker/target operands entirely.
        let short = registration_module(&[ValType::I32, ValType::I32, ValType::I32], &[0, 0]);
        let mut runtime = crate::Runtime::instantiate(&short).unwrap();
        runtime.call("reg", &[]).unwrap();
        assert!(runtime.state().embind.functions.is_empty());
        // Explicit null operands are equally undispatchable.
        let zeros = registration_module(
            &[
                ValType::I32,
                ValType::I32,
                ValType::I32,
                ValType::I32,
                ValType::I32,
                ValType::I32,
                ValType::I32,
            ],
            &[0, 0, 0, 0, 0, 0],
        );
        let mut runtime = crate::Runtime::instantiate(&zeros).unwrap();
        runtime.call("reg", &[]).unwrap();
        assert!(runtime.state().embind.functions.is_empty());
        // A complete registration still lands.
        let full = registration_module(
            &[
                ValType::I32,
                ValType::I32,
                ValType::I32,
                ValType::I32,
                ValType::I32,
                ValType::I32,
                ValType::I32,
            ],
            &[0, 0, 0, 7, 9, 0],
        );
        let mut runtime = crate::Runtime::instantiate(&full).unwrap();
        runtime.call("reg", &[]).unwrap();
        let functions = &runtime.state().embind.functions;
        assert_eq!(functions.len(), 1);
        assert_eq!((functions[0].invoker, functions[0].function), (7, 9));
    }
}
