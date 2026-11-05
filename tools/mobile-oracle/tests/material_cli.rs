//! Fabricated APK bytes only; exercises the real CLI, no Android execution.
#![cfg(unix)]
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::{
    fs,
    io::{Cursor, Write},
    path::Path,
    process::Command,
};
const NONE: u32 = u32::MAX;
fn tlv(tag: u8, b: &[u8]) -> Vec<u8> {
    assert!(b.len() < 128);
    let mut v = vec![tag, b.len() as u8];
    v.extend(b);
    v
}
fn signature_fixture() -> Vec<u8> {
    let mut signed = tlv(2, &[1]);
    signed.extend(tlv(0x31, &[]));
    signed.extend(tlv(0x30, &[]));
    signed.extend(tlv(0xa0, &[0x30, 1, 7]));
    signed.extend(tlv(0x31, &[]));
    let mut content = tlv(6, &[0x2a, 0x86, 0x48, 0x86, 0xf7, 0x0d, 1, 7, 2]);
    content.extend(tlv(0xa0, &tlv(0x30, &signed)));
    tlv(0x30, &content)
}
fn manifest_fixture() -> Vec<u8> {
    fn word(b: &mut Vec<u8>, v: u32) {
        b.extend(v.to_le_bytes());
    }
    fn half(b: &mut Vec<u8>, v: u16) {
        b.extend(v.to_le_bytes());
    }
    let values = [
        "manifest",
        "package",
        "com.whatsapp",
        "http://schemas.android.com/apk/res/android",
        "versionName",
        "2.0",
        "versionCode",
    ];
    let mut data = Vec::new();
    let mut offsets = Vec::new();
    for value in values {
        offsets.push(data.len() as u32);
        data.push(value.len() as u8);
        data.push(value.len() as u8);
        data.extend(value.as_bytes());
        data.push(0);
    }
    while data.len() % 4 != 0 {
        data.push(0);
    }
    let mut pool = Vec::new();
    half(&mut pool, 1);
    half(&mut pool, 28);
    word(&mut pool, (28 + offsets.len() * 4 + data.len()) as u32);
    word(&mut pool, offsets.len() as u32);
    word(&mut pool, 0);
    word(&mut pool, 256);
    word(&mut pool, (28 + offsets.len() * 4) as u32);
    word(&mut pool, 0);
    for offset in offsets {
        word(&mut pool, offset);
    }
    pool.extend(data);
    let mut start = Vec::new();
    half(&mut start, 0x102);
    half(&mut start, 16);
    word(&mut start, 96);
    word(&mut start, 1);
    word(&mut start, NONE);
    word(&mut start, NONE);
    word(&mut start, 0);
    half(&mut start, 20);
    half(&mut start, 20);
    half(&mut start, 3);
    half(&mut start, 0);
    half(&mut start, 0);
    half(&mut start, 0);
    for (ns, key, kind, value) in [(NONE, 1, 3, 2), (3, 4, 3, 5), (3, 6, 0x10, 42)] {
        word(&mut start, ns);
        word(&mut start, key);
        word(&mut start, NONE);
        half(&mut start, 8);
        start.push(0);
        start.push(kind);
        word(&mut start, value);
    }
    let mut end = Vec::new();
    half(&mut end, 0x103);
    half(&mut end, 16);
    word(&mut end, 24);
    word(&mut end, 1);
    word(&mut end, NONE);
    word(&mut end, NONE);
    word(&mut end, 0);
    let mut root = Vec::new();
    half(&mut root, 3);
    half(&mut root, 8);
    word(&mut root, (8 + pool.len() + start.len() + end.len()) as u32);
    root.extend(pool);
    root.extend(start);
    root.extend(end);
    root
}
fn invoke(
    lock: &Path,
    root: &Path,
    recipe: &Path,
    extra: &[&std::ffi::OsStr],
) -> std::process::Output {
    Command::new(env!("CARGO_BIN_EXE_mobile-oracle"))
        .arg("material")
        .arg(lock)
        .arg(root)
        .arg(recipe)
        .args(extra)
        .output()
        .unwrap()
}
#[test]
fn material_cli_exports_private_receipt_imports_and_rejects_bad_flags() {
    use std::os::unix::fs::PermissionsExt;
    let dir = tempfile::tempdir().unwrap();
    fs::write(dir.path().join(".gitignore"), "/.cache/\n").unwrap();
    let mut writer = zip::ZipWriter::new(Cursor::new(Vec::new()));
    for (name, bytes) in [
        ("AndroidManifest.xml", manifest_fixture()),
        ("classes.dex", b"synthetic-not-dex".to_vec()),
        ("logo.png", vec![0, 255, 7]),
        ("META-INF/SYNTHETIC.RSA", signature_fixture()),
    ] {
        writer
            .start_file(
                name,
                zip::write::SimpleFileOptions::default()
                    .compression_method(zip::CompressionMethod::Stored),
            )
            .unwrap();
        writer.write_all(&bytes).unwrap();
    }
    let apk = writer.finish().unwrap().into_inner();
    fs::write(dir.path().join("base.apk"), &apk).unwrap();
    let sig = b"synthetic untrusted signer report";
    fs::write(dir.path().join("signature.txt"), sig).unwrap();
    let lock = json!({"schema_version":1,"package_name":"com.whatsapp","version_name":"2.0","version_code":42,"abi":"arm64-v8a","min_sdk":23,"target_sdk":35,"distribution_channel":"synthetic","acquisition":{"source_url":"https://example.invalid/synthetic","acquired_at_utc":"2026-11-04T22:00:00Z","access_conditions":"synthetic only"},"signature":{"signer_certificate_sha256":"00".repeat(32),"trusted_signer_reference":"synthetic not trusted","verification_report_path":"signature.txt","verifier_version":"synthetic"},"declared_installation_complete":true,"artifacts":[{"path":"base.apk","kind":"base_apk","size_bytes":apk.len(),"sha256":hex::encode(Sha256::digest(&apk))},{"path":"signature.txt","kind":"signature_report","size_bytes":sig.len(),"sha256":hex::encode(Sha256::digest(sig))}]});
    let lock_bytes = serde_json::to_vec(&lock).unwrap();
    let lock_hash = hex::encode(Sha256::digest(&lock_bytes));
    let lock_path = dir.path().join("lock.json");
    fs::write(&lock_path, &lock_bytes).unwrap();
    let recipe = json!({"schema_version":1,"lock_sha256":lock_hash,"manifest":{"apk_path":"base.apk","member":"AndroidManifest.xml"},"dex":{"apk_path":"base.apk","member":"classes.dex"},"logo":{"apk_path":"base.apk","member":"logo.png"},"signatures":[{"apk_path":"base.apk","member":"META-INF/SYNTHETIC.RSA"}],"salt_hex":"090002","iterations":128});
    let recipe_bytes = serde_json::to_vec(&recipe).unwrap();
    let recipe_hash = hex::encode(Sha256::digest(&recipe_bytes));
    let recipe_path = dir.path().join("recipe.json");
    fs::write(&recipe_path, &recipe_bytes).unwrap();
    let normal = invoke(&lock_path, dir.path(), &recipe_path, &[]);
    assert!(normal.status.success(), "{:?}", normal);
    let normal: Value = serde_json::from_slice(&normal.stdout).unwrap();
    assert_eq!(normal["secret_exported"], false);
    assert!(normal.get("private_export").is_none());
    assert!(!dir.path().join(".cache").exists());
    let flag = std::ffi::OsStr::new("--export-private");
    let root = dir.path().as_os_str();
    for flags in [
        vec![flag],
        vec![flag, flag],
        vec![flag, std::ffi::OsStr::new("--unknown")],
        vec![std::ffi::OsStr::new("--unknown")],
        vec![flag, root, flag, root],
    ] {
        let output = invoke(&lock_path, dir.path(), &recipe_path, &flags);
        assert!(!output.status.success());
        assert!(output.stdout.is_empty());
        assert!(!dir.path().join(".cache").exists());
    }
    let output = invoke(&lock_path, dir.path(), &recipe_path, &[flag, root]);
    assert!(output.status.success(), "{:?}", output);
    let report: Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(report["secret_exported"], true);
    let receipt = &report["private_export"];
    let path = Path::new(receipt["path"].as_str().unwrap());
    assert!(path.starts_with(dir.path().join(".cache/mobile-oracle/private")));
    assert_eq!(
        fs::metadata(path).unwrap().permissions().mode() & 0o777,
        0o600
    );
    let material = mobile_oracle::import_private_material(
        path,
        receipt["sha256"].as_str().unwrap(),
        &lock_hash,
        &recipe_hash,
    )
    .unwrap();
    // Independently computed with Node standard crypto on these fabricated bytes.
    assert_eq!(
        material.token_base64("01234").unwrap(),
        "y9NTsqPR5qGBkkaaaynqOM4ke/w="
    );
    let private: Value = serde_json::from_slice(&fs::read(path).unwrap()).unwrap();
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert!(!stdout.contains(private["secret_hex"].as_str().unwrap()));
    assert!(!stdout.contains("090002"));
    assert!(output.stderr.is_empty());
    let repeat = invoke(&lock_path, dir.path(), &recipe_path, &[flag, root]);
    assert!(!repeat.status.success());
    assert!(repeat.stdout.is_empty());
}
