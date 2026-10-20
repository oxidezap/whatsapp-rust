#[path = "../build/descriptor.rs"]
mod descriptor;

use std::{
    fs,
    path::Path,
    time::{Duration, SystemTime},
};

fn modified(path: &Path) -> SystemTime {
    fs::metadata(path).unwrap().modified().unwrap()
}

#[test]
fn descriptor_cache_repairs_timestamps_and_preserves_content_changes() {
    let unique = SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let dir = std::env::temp_dir().join(format!(
        "waproto-descriptor-{}-{unique}",
        std::process::id()
    ));
    fs::create_dir(&dir).unwrap();
    let source = dir.join("source.desc");
    let output = dir.join("local.desc");
    fs::write(&source, b"source").unwrap();
    let past = SystemTime::now() - Duration::from_secs(60);
    let future = SystemTime::now() + Duration::from_secs(3600);
    let set_source = |time| {
        fs::File::options()
            .write(true)
            .open(&source)
            .unwrap()
            .set_modified(time)
            .unwrap()
    };
    set_source(past);
    descriptor::write(&output, b"first", &source).unwrap();
    assert_eq!(fs::read(&output).unwrap(), b"first");
    assert_eq!(modified(&output), modified(&source));

    // Matching content must repair an old cached descriptor with a future mtime.
    fs::File::options()
        .write(true)
        .open(&output)
        .unwrap()
        .set_modified(future)
        .unwrap();
    descriptor::write(&output, b"first", &source).unwrap();
    assert_eq!(modified(&output), modified(&source));
    let stable = modified(&output);
    descriptor::write(&output, b"first", &source).unwrap();
    assert_eq!(modified(&output), stable);

    // A future source must remain visible to Cargo, even with matching content.
    set_source(future);
    descriptor::write(&output, b"first", &source).unwrap();
    assert_eq!(modified(&output), modified(&source));
    assert!(modified(&output) > SystemTime::now());

    // Real encoded changes must replace the cache even when the source mtime is unchanged.
    descriptor::write(&output, b"second", &source).unwrap();
    assert_eq!(fs::read(&output).unwrap(), b"second");
    assert_eq!(modified(&output), modified(&source));
    set_source(past);
    descriptor::write(&output, b"third", &source).unwrap();
    assert_eq!(fs::read(&output).unwrap(), b"third");
    assert_eq!(modified(&output), modified(&source));
    fs::remove_dir_all(dir).unwrap();
}
