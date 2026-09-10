//! List every embind-registered function name and arity.
use anyhow::bail;
use oracle_core::{Catalog, Runtime, ThreadPolicy, Value};

fn main() -> anyhow::Result<()> {
    let catalog = Catalog::discover()?;
    let entry = catalog.resolve("JgwtTQVeWPm")?;
    let bytes = std::fs::read(&entry.path)?;
    let mut r = Runtime::instantiate(&bytes)?;
    r.set_thread_policy(ThreadPolicy::Spawn);
    r.set_main_thread_registration(true);
    r.run_ctors()?;
    r.refuel();
    let init = r.call_embind(
        "initVoipStack",
        &[
            Value::Str("15550002222@c.us".into()),
            Value::Str("15550002222:0@c.us".into()),
            Value::Str("99887766554433:0@lid".into()),
        ],
    );
    r.refuel();
    // Registrations happen during initialization: a failed init would list
    // whatever happened to register before that point as the complete set.
    if init.as_ref().ok().and_then(|v| v.as_int()) != Some(0) {
        bail!("initVoipStack failed: {init:?}");
    }
    println!("init: {init:?}");
    let mut names: Vec<String> = r
        .embind()
        .functions
        .iter()
        .map(|f| format!("{}({})", f.name, f.arg_types.len().saturating_sub(1)))
        .collect();
    names.sort();
    for name in names {
        println!("  {name}");
    }
    Ok(())
}
