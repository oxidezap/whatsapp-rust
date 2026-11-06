//! Native Rust Metro registry and the explicitly scoped legacy harness helpers.
//! Only separately verified original module bytes are evaluated.
use boa_engine::{
    Context, JsArgs, JsNativeError, JsResult, JsString, JsValue, NativeFunction, Source, js_string,
    object::{FunctionObjectBuilder, JsObject, ObjectInitializer},
    property::Attribute,
};
pub type Cx = Context;
pub fn string(s: &str) -> JsValue {
    JsString::from(s).into()
}
pub fn object(cx: &mut Cx) -> JsObject {
    ObjectInitializer::new(cx).build()
}
pub fn function(
    cx: &mut Cx,
    name: &str,
    f: boa_engine::native_function::NativeFunctionPointer,
) -> JsValue {
    FunctionObjectBuilder::new(cx.realm(), NativeFunction::from_fn_ptr(f))
        .name(JsString::from(name))
        .build()
        .into()
}
pub fn obj(v: &JsValue) -> JsResult<JsObject> {
    v.as_object()
        .ok_or_else(|| JsNativeError::typ().with_message("expected object").into())
}
pub fn get(o: &JsValue, name: &str, cx: &mut Cx) -> JsResult<JsValue> {
    obj(o)?.get(JsString::from(name), cx)
}
pub fn set(o: &JsValue, name: &str, value: JsValue, cx: &mut Cx) -> JsResult<()> {
    obj(o)?.set(JsString::from(name), value, true, cx)?;
    Ok(())
}
pub fn call(f: &JsValue, this: &JsValue, args: &[JsValue], cx: &mut Cx) -> JsResult<JsValue> {
    obj(f)?.call(this, args, cx)
}
pub fn method(o: &JsValue, name: &str, args: &[JsValue], cx: &mut Cx) -> JsResult<JsValue> {
    let f = get(o, name, cx)?;
    call(&f, o, args, cx)
}
pub fn identity(_: &JsValue, args: &[JsValue], _: &mut Cx) -> JsResult<JsValue> {
    Ok(args.get_or_undefined(0).clone())
}
pub fn false_fn(_: &JsValue, _: &[JsValue], _: &mut Cx) -> JsResult<JsValue> {
    Ok(false.into())
}
pub fn err(_: &JsValue, args: &[JsValue], cx: &mut Cx) -> JsResult<JsValue> {
    Ok(JsNativeError::error()
        .with_message(
            args.get_or_undefined(0)
                .to_string(cx)?
                .to_std_string_escaped(),
        )
        .into_opaque(cx)
        .into())
}
fn register(_: &JsValue, args: &[JsValue], cx: &mut Cx) -> JsResult<JsValue> {
    let registry = cx
        .global_object()
        .get(js_string!("__oracle_registry"), cx)?;
    let name = args.get_or_undefined(0).to_string(cx)?;
    let registry = obj(&registry)?;
    if registry.has_own_property(name.clone(), cx)? {
        return Err(JsNativeError::error()
            .with_message("duplicate module definition")
            .into());
    }
    registry.set(name, args.get_or_undefined(2).clone(), true, cx)?;
    Ok(JsValue::undefined())
}
fn require(_: &JsValue, args: &[JsValue], cx: &mut Cx) -> JsResult<JsValue> {
    load(
        &args
            .get_or_undefined(0)
            .to_string(cx)?
            .to_std_string_escaped(),
        cx,
    )
}
pub fn stub(name: &str, value: JsValue, cx: &mut Cx) -> JsResult<()> {
    let cache = cx.global_object().get(js_string!("__oracle_cache"), cx)?;
    set(&cache, name, value, cx)
}
pub fn load(name: &str, cx: &mut Cx) -> JsResult<JsValue> {
    let global = cx.global_object();
    let cache = global.get(js_string!("__oracle_cache"), cx)?;
    if obj(&cache)?.has_own_property(JsString::from(name), cx)? {
        return get(&cache, name, cx);
    }
    let registry = global.get(js_string!("__oracle_registry"), cx)?;
    let factory = get(&registry, name, cx)?;
    if factory.is_undefined() {
        return Err(JsNativeError::error()
            .with_message(format!("unexpected dependency: {name}"))
            .into());
    }
    let exports: JsValue = object(cx).into();
    let module: JsValue = object(cx).into();
    set(&module, "exports", exports.clone(), cx)?;
    set(&cache, name, exports.clone(), cx)?;
    let req = function(cx, "require", require);
    call(
        &factory,
        &JsValue::undefined(),
        &[
            global.into(),
            req.clone(),
            req.clone(),
            req,
            module.clone(),
            exports.clone(),
            exports,
        ],
        cx,
    )?;
    let result = get(&module, "exports", cx)?;
    set(&cache, name, result.clone(), cx)?;
    Ok(result)
}
fn inherits(_: &JsValue, args: &[JsValue], cx: &mut Cx) -> JsResult<JsValue> {
    let sub = args.get_or_undefined(0);
    let sup = args.get_or_undefined(1);
    let object = cx.global_object().get(js_string!("Object"), cx)?;
    let proto = get(sup, "prototype", cx)?;
    let p = method(&object, "create", &[proto], cx)?;
    set(&p, "constructor", sub.clone(), cx)?;
    set(sub, "prototype", p, cx)?;
    obj(sub)?.set_prototype(Some(obj(sup)?));
    Ok(JsValue::undefined())
}
fn without(_: &JsValue, args: &[JsValue], cx: &mut Cx) -> JsResult<JsValue> {
    let original = args.get_or_undefined(0);
    let excluded = args.get_or_undefined(1);
    let output: JsValue = object(cx).into();
    let object = cx.global_object().get(js_string!("Object"), cx)?;
    let mut current = original.clone();
    let mut seen = std::collections::BTreeSet::new();
    for _ in 0..64 {
        if current.is_null() {
            return Ok(output);
        }
        let keys = method(&object, "keys", &[current.clone()], cx)?;
        let n = get(&keys, "length", cx)?.to_u32(cx)?;
        if n > 4096 {
            return Err(JsNativeError::range()
                .with_message("helper key budget")
                .into());
        }
        for i in 0..n {
            let key = obj(&keys)?.get(i, cx)?;
            let name = key.to_string(cx)?.to_std_string_escaped();
            if seen.insert(name.clone()) && !method(excluded, "includes", &[key], cx)?.to_boolean()
            {
                set(&output, &name, get(original, &name, cx)?, cx)?;
            }
        }
        current = method(&object, "getPrototypeOf", &[current], cx)?;
    }
    Err(JsNativeError::range()
        .with_message("helper prototype budget")
        .into())
}
pub fn context() -> JsResult<Cx> {
    let mut cx = Context::default();
    cx.runtime_limits_mut().set_loop_iteration_limit(10_000_000);
    cx.runtime_limits_mut().set_recursion_limit(512);
    let registry = object(&mut cx);
    let cache = object(&mut cx);
    cx.register_global_property(js_string!("__oracle_registry"), registry, Attribute::all())?;
    cx.register_global_property(js_string!("__oracle_cache"), cache, Attribute::all())?;
    cx.register_global_builtin_callable(
        js_string!("__d"),
        4,
        NativeFunction::from_fn_ptr(register),
    )?;
    let helper: JsValue = object(&mut cx).into();
    for (name, f) in [
        (
            "inheritsLoose",
            inherits as boa_engine::native_function::NativeFunctionPointer,
        ),
        ("assertThisInitialized", identity),
        ("taggedTemplateLiteralLoose", identity),
        ("objectWithoutPropertiesLoose", without),
    ] {
        let value = function(&mut cx, name, f);
        set(&helper, name, value, &mut cx)?;
    }
    let object = cx.global_object().get(js_string!("Object"), &mut cx)?;
    let assign = get(&object, "assign", &mut cx)?;
    set(&helper, "extends", assign, &mut cx)?;
    cx.register_global_property(js_string!("babelHelpers"), helper, Attribute::all())?;
    let error = function(&mut cx, "err", err);
    stub("err", error, &mut cx)?;
    Ok(cx)
}
pub fn evaluate(bytes: &[u8], cx: &mut Cx) -> JsResult<()> {
    cx.eval(Source::from_bytes(bytes))?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn undeclared_dependencies_fail_instead_of_becoming_empty_mocks() {
        let mut cx = context().unwrap();
        assert!(load("unexpected-client-dependency", &mut cx).is_err());
    }
}
