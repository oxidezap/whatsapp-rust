use crate::descriptor::FileDescriptorSet;
use std::io;

pub fn prepare(bytes: &[u8]) -> io::Result<(FileDescriptorSet, crate::schema::Schema)> {
    let mut descriptors = FileDescriptorSet::decode_from_slice(bytes)?;
    crate::names::apply(
        &mut descriptors,
        crate::names::TYPES,
        crate::names::FIELDS,
        crate::names::ENUM_VALUES,
        crate::names::ONEOFS,
    )?;
    crate::retained::apply_local_fields(&mut descriptors)?;
    crate::wallpaper::retain_fields(&mut descriptors)?;
    crate::signal_storage::check(&crate::names::wire_api(&descriptors))?;
    let schema = crate::project::project(&descriptors).map_err(io::Error::other)?;
    crate::media_retry_schema::validate(&schema).map_err(io::Error::other)?;
    Ok((descriptors, schema))
}
