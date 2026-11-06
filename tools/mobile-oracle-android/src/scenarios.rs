use super::*;
use base64::Engine as _;
use std::{fs, io::Read};
fn hex_bytes(s: &str) -> Result<Vec<u8>> {
    hex::decode(s).map_err(anyhow::Error::msg)
}
fn obs(id: &str, bytes: &[u8]) -> Value {
    json!({"id":id,"output_hex":hex::encode(bytes),"output_sha256":digest(bytes),"output_base64":base64::engine::general_purpose::STANDARD.encode(bytes)})
}
pub(super) fn crypto(c: &mut Calls<'_, '_>) -> Result<Value> {
    let mut token_message = vec![0x30, 1, 7];
    token_message.extend(0..16);
    token_message.extend(b"0000000000");
    let cases = [
        (vec![0x0b; 20], b"Hi There".to_vec()),
        (b"Jefe".to_vec(), b"what do ya want for nothing?".to_vec()),
        (vec![0x80, 0xff, 0, 1], vec![]),
        ((0..64).collect(), token_message),
    ];
    let mut results = vec![];
    for (i, (key, message)) in cases.iter().enumerate() {
        let k = c.bytes(key)?;
        let m = c.bytes(message)?;
        let parts = c.array("[B", &[m])?;
        let output = c
            .stat(
                "X/00M",
                "A0M",
                "([B[[B)[B",
                &[JValue::Object(&k), JValue::Object(&parts)],
            )?
            .l()?;
        let output = c.output(output)?;
        let mut result = obs(&format!("synthetic-{i}"), &output);
        result["key_sha256"] = json!(digest(key));
        result["message_sha256"] = json!(digest(message));
        results.push(result);
    }
    ensure!(
        results[0]["output_hex"] == "b617318655057264e28bc0b6fb378c8ef146be00"
            && results[1]["output_hex"] == "effcdf6ae5eb2fa2d27416d5f184df9c259a7c79",
        "RFC2202 mismatch"
    );
    let key = c.bytes(&[11; 20])?;
    let empty = c.array("[B", &[])?;
    ensure!(
        c.stat(
            "X/00M",
            "A0M",
            "([B[[B)[B",
            &[JValue::Object(&key), JValue::Object(&empty)]
        )
        .is_err(),
        "empty parts accepted"
    );
    ensure!(
        c.exception()? == "java.lang.ArrayIndexOutOfBoundsException",
        "empty-parts error changed"
    );
    let mut password = b"com.whatsapp".to_vec();
    password.extend([0, 255, 128, 1]);
    let chars: Vec<u16> = password.iter().map(|b| (*b as i8) as i16 as u16).collect();
    let char_array = c.env.new_char_array(chars.len())?;
    char_array.set_region(c.env, 0, &chars)?;
    let char_object: JObject<'_> = char_array.into();
    let salt: Vec<u8> = (0..16).collect();
    let s = c.bytes(&salt)?;
    let algorithm = c.s("PBKDF2WithHmacSHA1And8BIT")?;
    let secret = c
        .stat(
            "X/00M",
            "A08",
            "(Ljava/lang/String;[B[CII)Ljavax/crypto/SecretKey;",
            &[
                JValue::Object(&algorithm),
                JValue::Object(&s),
                JValue::Object(&char_object),
                JValue::Int(128),
                JValue::Int(512),
            ],
        )?
        .l()?;
    let encoded = c.call(&secret, "getEncoded", "()[B", &[])?.l()?;
    let encoded = c.output(encoded)?;
    Ok(
        json!({"execution":"android_art_original_dex","class_initialization":"original","whatsapp_jni":"not_run","cases":results,"empty_parts":"array_bounds_error","pbkdf":{"id":"synthetic-binary-password-8bit","iterations":128,"bits":512,"output_hex":hex::encode(encoded),"password_sha256":digest(&password),"salt_sha256":digest(&salt)}}),
    )
}
pub(super) fn superpack(c: &mut Calls<'_, '_>, config: &Value, decompress: bool) -> Result<Value> {
    let lib = pinned(config, "library", "library_sha256")?;
    c.load(&lib)?;
    let class = "com/facebook/superpack/AssetDecompressor";
    if !decompress {
        let marker = c.bytes(&[0; 8])?;
        c.stat(
            class,
            "testDecompressorLibraryUsable",
            "([B)V",
            &[JValue::Object(&marker)],
        )?;
        let value = c.output(marker)?;
        ensure!(
            value == [0x47, 0x77, 0x53, 0, 0, 0, 0, 0],
            "native marker mismatch"
        );
        let short = c.bytes(&[0; 2])?;
        ensure!(
            c.stat(
                class,
                "testDecompressorLibraryUsable",
                "([B)V",
                &[JValue::Object(&short)]
            )
            .is_err(),
            "short array accepted"
        );
        ensure!(
            c.exception()? == "java.lang.ArrayIndexOutOfBoundsException",
            "short-array error changed"
        );
        let arch = c.stat(class, "get_architecture", "()I", &[])?.i()?;
        return Ok(
            json!({"execution":"android_art_original_jni","library_sha256":required(config,"library_sha256")?,"marker_hex":hex::encode(value),"architecture_result":arch,"undersized_array":"array_bounds_error","decompression":"not_run","protocol_crypto":"not_run"}),
        );
    }
    let container = pinned(config, "container", "container_sha256")?;
    let out = Path::new(required(config, "output")?);
    ensure!(
        !out.exists()
            && out
                .parent()
                .context("output parent")?
                .canonicalize()?
                .starts_with("/data/local/tmp/mobile-oracle"),
        "fresh private output required"
    );
    fs::create_dir(out)?;
    let a = c.s(container.to_str().context("UTF8 container")?)?;
    let format = c.s("spo")?;
    let path = c.s(out.to_str().context("UTF8 output")?)?;
    let names = c
        .stat(
            class,
            "decompress_range_from_so",
            "(Ljava/lang/String;Ljava/lang/String;Ljava/lang/String;II)[Ljava/lang/String;",
            &[
                JValue::Object(&a),
                JValue::Object(&format),
                JValue::Object(&path),
                JValue::Int(0),
                JValue::Int(18),
            ],
        )?
        .l()?;
    let names = JObjectArray::<JString>::cast_local(c.env, names)?;
    ensure!(names.len(c.env)? == 18, "native result count");
    let mut entries = fs::read_dir(out)?.collect::<std::io::Result<Vec<_>>>()?;
    entries.sort_by_key(|e| e.file_name());
    ensure!(entries.len() == 18, "native file count");
    let mut artifacts = vec![];
    let mut total = 0;
    for entry in entries {
        let path = entry.path();
        ensure!(
            path.is_file() && path.canonicalize()?.parent() == Some(out.canonicalize()?.as_path()),
            "unsafe extracted output"
        );
        let size = path.metadata()?.len();
        total += size;
        ensure!(total <= 512 * 1024 * 1024, "aggregate output budget");
        artifacts.push(json!({"name":entry.file_name().to_str().context("UTF8 file")?,"size_bytes":size,"sha256":hash_file(&path,64*1024*1024)?}));
    }
    Ok(
        json!({"execution":"android_original_superpack_jni","range_start":0,"range_count":18,"native_result_count":18,"artifacts":artifacts}),
    )
}
pub(super) fn curve(c: &mut Calls<'_, '_>, config: &Value) -> Result<Value> {
    let lib = pinned(config, "library", "library_sha256")?;
    c.load(&lib)?;
    let class = "org/whispersystems/curve25519/NativeCurve25519Provider";
    let p = c.construct(class, "()V", &[])?;
    let alice = hex_bytes("77076d0a7318a57d3c16c17251b26645df4c2f87ebc0992ab177fba51db92c2a")?;
    let bob = hex_bytes("5dab087e624a8a4b79e17f8b83800ee66f3bb1292618b6fd1c2f8b27ff88e0eb")?;
    let a = c.bytes(&alice)?;
    let b = c.bytes(&bob)?;
    let a = c
        .call(&p, "generatePrivateKey", "([B)[B", &[JValue::Object(&a)])?
        .l()?;
    let b = c
        .call(&p, "generatePrivateKey", "([B)[B", &[JValue::Object(&b)])?
        .l()?;
    let ap = c
        .call(&p, "generatePublicKey", "([B)[B", &[JValue::Object(&a)])?
        .l()?;
    let bp = c
        .call(&p, "generatePublicKey", "([B)[B", &[JValue::Object(&b)])?
        .l()?;
    let shared = c
        .call(
            &p,
            "calculateAgreement",
            "([B[B)[B",
            &[JValue::Object(&a), JValue::Object(&bp)],
        )?
        .l()?;
    let reverse = c
        .call(
            &p,
            "calculateAgreement",
            "([B[B)[B",
            &[JValue::Object(&b), JValue::Object(&ap)],
        )?
        .l()?;
    let shared = c.output(shared)?;
    ensure!(shared == c.output(reverse)?, "DH symmetry mismatch");
    let message = c.bytes(b"mobile oracle synthetic")?;
    let random = c.bytes(&[0; 64])?;
    let sig = c
        .call(
            &p,
            "calculateSignature",
            "([B[B[B)[B",
            &[
                JValue::Object(&random),
                JValue::Object(&a),
                JValue::Object(&message),
            ],
        )?
        .l()?;
    ensure!(
        c.call(
            &p,
            "verifySignature",
            "([B[B[B)Z",
            &[
                JValue::Object(&ap),
                JValue::Object(&message),
                JValue::Object(&sig)
            ]
        )?
        .z()?,
        "native signature invalid"
    );
    let mut altered = b"mobile oracle synthetic".to_vec();
    altered[0] ^= 1;
    let changed = c.bytes(&altered)?;
    ensure!(
        !c.call(
            &p,
            "verifySignature",
            "([B[B[B)Z",
            &[
                JValue::Object(&ap),
                JValue::Object(&changed),
                JValue::Object(&sig)
            ]
        )?
        .z()?,
        "altered message accepted"
    );
    let torsion = c
        .call(&p, "isTorsionFree", "([B)Z", &[JValue::Object(&ap)])?
        .z()?;
    let zero = c.bytes(&[0; 32])?;
    let low = match c.call(
        &p,
        "calculateAgreement",
        "([B[B)[B",
        &[JValue::Object(&a), JValue::Object(&zero)],
    ) {
        Ok(v) => json!({"status":"returned","output_hex":hex::encode(c.output(v.l()?)?)}),
        Err(_) => json!({"status":"threw","exception":c.exception()?}),
    };
    let ap = c.output(ap)?;
    let bp = c.output(bp)?;
    let sig = c.output(sig)?;
    ensure!(
        hex::encode(&ap) == "8520f0098930a754748b7ddcb43ef75a0dbf3a0d26381af4eba4a98eaa9b4e6a"
            && hex::encode(&shared)
                == "4a5d9d5ba4ce2de1728e3bf480350f25e07e21c947d19e3376f09b3c1e161742",
        "RFC7748 mismatch"
    );
    Ok(
        json!({"execution":"android_original_curve_jni","library_sha256":required(config,"library_sha256")?,"randomness":"explicit_synthetic_zero_64_bytes","cases":[obs("alice-public",&ap),obs("bob-public",&bp),obs("shared",&shared),obs("signature",&sig)],"signature_valid":true,"tampered_signature_valid":false,"alice_torsion_free":torsion,"low_order_agreement":low}),
    )
}
fn body<'e>(c: &mut Calls<'_, 'e>, builder: &JObject<'e>) -> Result<String> {
    let formatter = c.construct(
        "X/KrQ",
        "(Ljava/lang/Object;I)V",
        &[JValue::Object(builder), JValue::Int(46)],
    )?;
    let map = c.field(builder, "A00", "Ljava/util/Map;")?;
    let entries = c.call(&map, "entrySet", "()Ljava/util/Set;", &[])?.l()?;
    let iter = c
        .call(&entries, "iterator", "()Ljava/util/Iterator;", &[])?
        .l()?;
    let mut parts = vec![];
    while c.call(&iter, "hasNext", "()Z", &[])?.z()? {
        ensure!(parts.len() < 32, "field budget");
        let item = c.call(&iter, "next", "()Ljava/lang/Object;", &[])?.l()?;
        let text = c
            .call(
                &formatter,
                "invoke",
                "(Ljava/lang/Object;)Ljava/lang/Object;",
                &[JValue::Object(&item)],
            )?
            .l()?;
        parts.push(c.text(text)?);
    }
    Ok(parts.join("&"))
}
pub(super) fn form(c: &mut Calls<'_, '_>, apk: &Path) -> Result<Value> {
    let mut zip = zip::ZipArchive::new(File::open(apk)?)?;
    let mut dex = zip.by_name("classes10.dex")?;
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
    let dex_hash = hex::encode(h.finalize());
    let raw = c.construct("X/KCq", "()V", &[])?;
    let name = c.s("probe_blob")?;
    let all: Vec<u8> = (0..=255).collect();
    let bytes = c.bytes(&all)?;
    c.call(
        &raw,
        "A06",
        "(Ljava/lang/String;[B)V",
        &[JValue::Object(&name), JValue::Object(&bytes)],
    )?;
    let raw_body = body(c, &raw)?;
    let flags = c.field(&raw, "A01", "Ljava/util/Set;")?;
    ensure!(
        c.call(
            &flags,
            "contains",
            "(Ljava/lang/Object;)Z",
            &[JValue::Object(&name)]
        )?
        .z()?,
        "encoded flag missing"
    );
    let b64 = c.construct("X/KCq", "()V", &[])?;
    c.call(
        &b64,
        "A04",
        "(Ljava/lang/String;[B)V",
        &[JValue::Object(&name), JValue::Object(&bytes)],
    )?;
    let b64_body = body(c, &b64)?;
    let mixed = c.construct("X/KCq", "()V", &[])?;
    let label = c.s("label")?;
    let text = c.s("synthetic space +%&/é~*")?;
    c.call(
        &mixed,
        "A01",
        "(Ljava/lang/String;Ljava/lang/String;)V",
        &[JValue::Object(&label), JValue::Object(&text)],
    )?;
    for (key, n) in [("yes", 1), ("no", 0), ("omitted_bool", 2)] {
        let key = c.s(key)?;
        c.call(
            &mixed,
            "A00",
            "(Ljava/lang/String;I)V",
            &[JValue::Object(&key), JValue::Int(n)],
        )?;
    }
    let key = c.s("omitted_text")?;
    c.call(
        &mixed,
        "A02",
        "(Ljava/lang/String;Ljava/lang/String;)V",
        &[JValue::Object(&key), JValue::Object(&JObject::null())],
    )?;
    let key = c.s("omitted_bytes")?;
    c.call(
        &mixed,
        "A05",
        "(Ljava/lang/String;[B)V",
        &[JValue::Object(&key), JValue::Object(&JObject::null())],
    )?;
    let key = c.s("probe_uuid")?;
    let uuid = c.s("00010203-0405-0607-0809-0a0b0c0d0e0f")?;
    c.call(
        &mixed,
        "A03",
        "(Ljava/lang/String;Ljava/lang/String;)V",
        &[JValue::Object(&key), JValue::Object(&uuid)],
    )?;
    let mixed_body = body(c, &mixed)?;
    let report_body =
        |id: &str, text: String| json!({"id":id,"body_sha256":digest(text.as_bytes()),"body":text});
    Ok(
        json!({"execution":"android_original_registration_form","http":"not_called","reference_dex_sha256":dex_hash,"formatter_profile":46,"encoded_flag_retained":true,"corpus":"all_256_bytes_and_synthetic_text","cases":[report_body("raw-all-bytes",raw_body),report_body("base64-all-bytes",b64_body),report_body("optional-text-uuid",mixed_body)]}),
    )
}
