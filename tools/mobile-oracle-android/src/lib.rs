//! Rust-owned JNI harness. Original APK code remains the external reference.
//! Run only through the generated DEX entry in a disposable, offline Android VM.
use anyhow::{Context, Result, bail, ensure};
use jni::{
    Env, EnvUnowned, JValue, JValueOwned,
    objects::{JByteArray, JClass, JObject, JObjectArray, JString},
    signature::{RuntimeFieldSignature, RuntimeMethodSignature},
    strings::JNIString,
};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::{
    fs::File,
    io::{Read, Write},
    path::Path,
};

mod scenarios;

fn digest(bytes: &[u8]) -> String {
    hex::encode(Sha256::digest(bytes))
}
fn hash_file(path: &Path, limit: u64) -> Result<String> {
    ensure!(
        path.is_file() && path.metadata()?.len() <= limit,
        "file budget"
    );
    let mut file = File::open(path)?;
    let mut h = Sha256::new();
    let mut bytes = [0u8; 16384];
    let mut total = 0;
    loop {
        let n = file.read(&mut bytes)?;
        if n == 0 {
            break;
        }
        total += n as u64;
        ensure!(total <= limit, "file grew beyond budget");
        h.update(&bytes[..n]);
    }
    Ok(hex::encode(h.finalize()))
}
fn private_path(value: &str) -> Result<std::path::PathBuf> {
    let p = Path::new(value).canonicalize()?;
    ensure!(
        p.starts_with("/data/local/tmp/mobile-oracle"),
        "path leaves private guest capture"
    );
    Ok(p)
}
fn required<'a>(v: &'a Value, key: &str) -> Result<&'a str> {
    v.get(key)
        .and_then(Value::as_str)
        .with_context(|| format!("missing {key}"))
}
fn pinned(config: &Value, path: &str, hash: &str) -> Result<std::path::PathBuf> {
    let p = private_path(required(config, path)?)?;
    ensure!(
        hash_file(&p, 256 * 1024 * 1024)? == required(config, hash)?,
        "{path} hash mismatch"
    );
    Ok(p)
}
struct Calls<'a, 'e> {
    env: &'a mut Env<'e>,
}
impl<'e> Calls<'_, 'e> {
    fn s(&mut self, value: &str) -> Result<JObject<'e>> {
        Ok(self.env.new_string(value)?.into())
    }
    fn bytes(&mut self, value: &[u8]) -> Result<JObject<'e>> {
        Ok(self.env.byte_array_from_slice(value)?.into())
    }
    fn text(&mut self, value: JObject<'e>) -> Result<String> {
        let s = JString::cast_local(self.env, value)?;
        Ok(s.try_to_string(self.env)?)
    }
    fn output(&mut self, value: JObject<'e>) -> Result<Vec<u8>> {
        let b = JByteArray::cast_local(self.env, value)?;
        Ok(self.env.convert_byte_array(b)?)
    }
    fn call(
        &mut self,
        obj: &JObject<'e>,
        name: &str,
        sig: &str,
        args: &[JValue<'_>],
    ) -> Result<JValueOwned<'e>> {
        let sig = RuntimeMethodSignature::from_str(sig)?;
        Ok(self
            .env
            .call_method(obj, JNIString::new(name), sig.method_signature(), args)?)
    }
    fn stat(
        &mut self,
        class: &str,
        name: &str,
        sig: &str,
        args: &[JValue<'_>],
    ) -> Result<JValueOwned<'e>> {
        let sig = RuntimeMethodSignature::from_str(sig)?;
        Ok(self.env.call_static_method(
            JNIString::new(class),
            JNIString::new(name),
            sig.method_signature(),
            args,
        )?)
    }
    fn construct(&mut self, class: &str, sig: &str, args: &[JValue<'_>]) -> Result<JObject<'e>> {
        let sig = RuntimeMethodSignature::from_str(sig)?;
        Ok(self
            .env
            .new_object(JNIString::new(class), sig.method_signature(), args)?)
    }
    fn field(&mut self, obj: &JObject<'e>, name: &str, sig: &str) -> Result<JObject<'e>> {
        let sig = RuntimeFieldSignature::from_str(sig)?;
        Ok(self
            .env
            .get_field(obj, JNIString::new(name), sig.field_signature())?
            .l()?)
    }
    fn static_field(&mut self, class: &str, name: &str, sig: &str) -> Result<JValueOwned<'e>> {
        let sig = RuntimeFieldSignature::from_str(sig)?;
        Ok(self.env.get_static_field(
            JNIString::new(class),
            JNIString::new(name),
            sig.field_signature(),
        )?)
    }
    fn exception(&mut self) -> Result<String> {
        let exception = self
            .env
            .exception_occurred()
            .context("expected original exception")?;
        self.env.exception_clear();
        let class = self
            .call(exception.as_ref(), "getClass", "()Ljava/lang/Class;", &[])?
            .l()?;
        let name = self
            .call(&class, "getName", "()Ljava/lang/String;", &[])?
            .l()?;
        self.text(name)
    }
    fn load(&mut self, path: &Path) -> Result<()> {
        let s = self.s(path.to_str().context("UTF8 native path")?)?;
        self.stat(
            "java/lang/System",
            "load",
            "(Ljava/lang/String;)V",
            &[JValue::Object(&s)],
        )?;
        Ok(())
    }
    fn fingerprint(&mut self) -> Result<String> {
        let value = self
            .static_field("android/os/Build", "FINGERPRINT", "Ljava/lang/String;")?
            .l()?;
        self.text(value)
    }
    fn array(&mut self, class: &str, objects: &[JObject<'e>]) -> Result<JObject<'e>> {
        let a = self.env.new_object_array(
            objects.len() as i32,
            JNIString::new(class),
            JObject::null(),
        )?;
        for (i, o) in objects.iter().enumerate() {
            a.set_element(self.env, i, o)?;
        }
        Ok(a.into())
    }
}
fn run(env: &mut Env<'_>, args: JObjectArray<'_, JString<'_>>) -> Result<Value> {
    ensure!(
        args.len(env)? == 1,
        "pass one private JSON configuration path"
    );
    let path = args.get_element(env, 0)?.try_to_string(env)?;
    let path = private_path(&path)?;
    ensure!(path.metadata()?.len() <= 65536, "configuration size budget");
    let config: Value = serde_json::from_slice(&std::fs::read(path)?)?;
    let scenario = required(&config, "scenario")?;
    let mut calls = Calls { env };
    let vm_key = calls.s("java.vm.name")?;
    let vm = calls
        .stat(
            "java/lang/System",
            "getProperty",
            "(Ljava/lang/String;)Ljava/lang/String;",
            &[JValue::Object(&vm_key)],
        )?
        .l()?;
    ensure!(
        calls.text(vm)? == "Dalvik",
        "requires actual Android runtime"
    );
    if scenario == "self-test" {
        return Ok(json!({"execution":"rust_jni_self_test","qualified_android_reference":false}));
    }
    let apk = pinned(&config, "apk", "apk_sha256")?;
    // The only managed reference input is the exact pinned APK on CLASSPATH.
    let cp = std::env::var("CLASSPATH").context("CLASSPATH")?;
    let paths: Vec<_> = cp.split(':').collect();
    ensure!(
        paths.len() == 2 && Path::new(paths[1]) == apk,
        "classpath must be bootstrap plus pinned APK"
    );
    let mut report = match scenario {
        "crypto" => scenarios::crypto(&mut calls)?,
        "superpack" => scenarios::superpack(&mut calls, &config, false)?,
        "decompress" => scenarios::superpack(&mut calls, &config, true)?,
        "curve" => scenarios::curve(&mut calls, &config)?,
        "form" => scenarios::form(&mut calls, &apk)?,
        _ => bail!("unknown offline scenario"),
    };
    if scenario == "crypto" {
        let mut zip = zip::ZipArchive::new(File::open(&apk)?)?;
        let mut dex = zip.by_name("classes.dex")?;
        ensure!(dex.size() <= 64 * 1024 * 1024, "DEX budget");
        let mut h = Sha256::new();
        let mut buffer = [0; 16384];
        loop {
            let n = dex.read(&mut buffer)?;
            if n == 0 {
                break;
            }
            h.update(&buffer[..n]);
        }
        report["dex_sha256"] = json!(hex::encode(h.finalize()));
    }
    report["driver"] = json!("rust_jni");
    report["schema_version"] = json!(1);
    report["stubs"] = json!([]);
    report["application_lifecycle"] = json!("not_run");
    report["registration"] = json!("not_run");
    report["build_fingerprint"] = json!(calls.fingerprint()?);
    Ok(report)
}

/// ART supplies the attached environment and managed references. jni-rs catches
/// panics at this boundary; errors become real RuntimeExceptions, never reports.
#[unsafe(no_mangle)]
pub extern "system" fn Java_org_oxidezap_oracle_RustRunner_main<'e>(
    mut unowned: EnvUnowned<'e>,
    _class: JClass<'e>,
    args: JObjectArray<'e, JString<'e>>,
) {
    unowned
        .with_env(|env| -> std::result::Result<(), NativeError> {
            match run(env, args) {
                Ok(report) => {
                    let output =
                        serde_json::to_string(&report).map_err(|e| NativeError(e.to_string()))?;
                    writeln!(std::io::stdout().lock(), "{output}")
                        .map_err(|e| NativeError(e.to_string()))
                }
                Err(error) => {
                    if env.exception_occurred().is_some() {
                        env.exception_describe();
                        env.exception_clear();
                    }
                    Err(NativeError(format!("{error:#}")))
                }
            }
        })
        .resolve::<jni::errors::ThrowRuntimeExAndDefault>()
}

#[derive(Debug)]
struct NativeError(String);
impl std::fmt::Display for NativeError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.0.fmt(f)
    }
}
impl std::error::Error for NativeError {}
impl From<jni::errors::Error> for NativeError {
    fn from(error: jni::errors::Error) -> Self {
        Self(error.to_string())
    }
}
