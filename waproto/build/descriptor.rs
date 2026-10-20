use std::path::Path;

pub fn write(path: &Path, encoded: &[u8], source: &Path) -> std::io::Result<()> {
    if !std::fs::read(path).is_ok_and(|current| current == encoded) {
        std::fs::write(path, encoded)?;
    }
    // Repair cached descriptors too, including matching files dated in the future.
    let source_time = std::fs::metadata(source)?.modified()?;
    let file = std::fs::File::options().write(true).open(path)?;
    if file.metadata()?.modified()? != source_time {
        file.set_modified(source_time)?;
    }
    Ok(())
}
