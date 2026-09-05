//! Each `_embind_register_*` import has its own argument layout, and reading
//! one with another's is silent.
//!
//! No captured module declares `_embind_register_class_property` or
//! `_embind_register_class_class_function`, so the corpus cannot answer this —
//! which is exactly why it is a fixture. The registrations are made by a
//! hand-built module, so the test says precisely which arguments the host saw.

use oracle_core::Runtime;
use wasm_encoder::{
    CodeSection, ConstExpr, DataSection, EntityType, ExportKind, ExportSection, Function,
    FunctionSection, ImportSection, MemorySection, MemoryType, Module, TypeSection, ValType,
};

/// Type id the fixture registers its class under.
const CLASS_TYPE: i32 = 100;
/// Type id of the property's value.
const INT_TYPE: i32 = 200;

/// Where the names sit. Not at address 0: a null pointer is how embind spells
/// "no string", so `read_cstr` answers an empty one there — and a fixture whose
/// class name lands on it tests nothing.
const NAMES_AT: i32 = 16;
const NAME_SHAPE: i32 = NAMES_AT;
const NAME_WIDTH: i32 = NAMES_AT + 6;
const NAME_INT: i32 = NAMES_AT + 12;
const NAMES: &[u8] = b"Shape\0width\0int\0";

/// A module that registers one class with one data member, and nothing else.
fn fixture() -> Vec<u8> {
    let i32s = |count: usize| vec![ValType::I32; count];

    let mut types = TypeSection::new();
    types.ty().function(i32s(13), []); // 0: _embind_register_class
    types.ty().function(i32s(10), []); // 1: _embind_register_class_property
    types.ty().function(i32s(5), []); // 2: _embind_register_integer
    types.ty().function([], []); // 3: the fixture's own entry point

    let mut imports = ImportSection::new();
    imports.import("env", "_embind_register_class", EntityType::Function(0));
    imports.import(
        "env",
        "_embind_register_class_property",
        EntityType::Function(1),
    );
    imports.import("env", "_embind_register_integer", EntityType::Function(2));

    let mut memories = MemorySection::new();
    memories.memory(MemoryType {
        minimum: 1,
        maximum: None,
        memory64: false,
        shared: false,
        page_size_log2: None,
    });

    let mut functions = FunctionSection::new();
    functions.function(3);

    let mut body = Function::new([]);
    {
        let mut f = body.instructions();

        // _embind_register_integer(type, name, size, min, max)
        for value in [INT_TYPE, NAME_INT, 4, i32::MIN, i32::MAX] {
            f.i32_const(value);
        }
        f.call(2);

        // _embind_register_class(classType, pointerType, constPointerType,
        //   baseType, getActualTypeSignature, getActualType, upcastSignature,
        //   upcast, downcastSignature, downcast, name, destructorSignature,
        //   destructor)
        for value in [CLASS_TYPE, 0, 0, 0, 0, 0, 0, 0, 0, 0, NAME_SHAPE, 0, 77] {
            f.i32_const(value);
        }
        f.call(0);

        // _embind_register_class_property(classType, fieldName,
        //   getterReturnType, getterSignature, getter, getterContext,
        //   setterArgumentType, setterSignature, setter, setterContext)
        //
        // The distinctive values are the ones the method layout would have
        // misread: argument 2 is a *type*, not a count, and 3 is a signature
        // string, not an array of type ids.
        for value in [
            CLASS_TYPE, NAME_WIDTH, INT_TYPE, 900, 11, 22, INT_TYPE, 901, 33, 44,
        ] {
            f.i32_const(value);
        }
        f.call(1);

        f.end();
    }

    let mut code = CodeSection::new();
    code.function(&body);

    let mut data = DataSection::new();
    data.active(0, &ConstExpr::i32_const(NAMES_AT), NAMES.iter().copied());

    let mut exports = ExportSection::new();
    exports.export("memory", ExportKind::Memory, 0);
    // The name `run_ctors` looks for, so the registrations happen the way a
    // real emscripten module's would.
    exports.export("__wasm_call_ctors", ExportKind::Func, 3);

    let mut module = Module::new();
    module.section(&types);
    module.section(&imports);
    module.section(&functions);
    module.section(&memories);
    module.section(&exports);
    module.section(&code);
    module.section(&data);
    module.finish()
}

/// A property is a field, and recording it as a method offered a callable that
/// does not exist: the method layout reads argument 2 as an argument *count*
/// and argument 3 as a pointer to type ids, when they are the field's type and
/// a signature string. The invoker and context it recorded were the getter's
/// signature pointer and the getter itself, so calling it would have dispatched
/// to the wrong table entry.
#[test]
fn a_class_property_is_read_as_a_field_and_not_as_a_method() {
    let bytes = fixture();
    let mut runtime = Runtime::instantiate(&bytes).expect("instantiate");
    runtime.run_ctors().expect("ctors");

    let registry = runtime.embind();
    let class = registry
        .classes
        .values()
        .find(|class| class.name == "Shape")
        .expect("the fixture registers `Shape`");

    assert!(
        class.methods.is_empty(),
        "a property is not a callable method: {:?}",
        class.methods
    );
    assert_eq!(class.properties.len(), 1, "one data member");

    let property = &class.properties[0];
    assert_eq!(property.name, "width");
    assert_eq!(
        registry.type_name(property.field_type),
        "int",
        "argument 2 is the field's type, not an argument count"
    );
    assert_eq!((property.getter, property.getter_context), (11, 22));
    assert_eq!(property.setter_type, Some(INT_TYPE as u32));
    assert_eq!((property.setter, property.setter_context), (33, 44));

    assert_eq!(registry.property_count(), 1);
}
