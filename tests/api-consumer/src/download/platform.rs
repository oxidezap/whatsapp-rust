//! Platform contracts for browser-local adapters and native task spawning.

use std::future::Future;
use std::io::Cursor;
use whatsapp_rust::Client;
use whatsapp_rust::download::{Downloadable, MediaDownloader};

#[cfg(not(target_arch = "wasm32"))]
fn send(_: impl Future + Send) {}

#[cfg(not(target_arch = "wasm32"))]
pub fn native_futures(client: &Client, downloader: &MediaDownloader, media: &dyn Downloadable) {
    send(client.download(media));
    send(client.download_to_writer(media, Cursor::new(Vec::new())));
    send(downloader.download(media));
    send(downloader.download_to_writer(media, Cursor::new(Vec::new())));
}

#[cfg(target_arch = "wasm32")]
mod browser {
    use super::*;
    use std::cell::RefCell;
    use std::io::{self, Seek, SeekFrom, Write};
    use std::rc::Rc;
    use whatsapp_rust::download::{DownloadWriter, MediaType};

    pub struct LocalMedia {
        pub path: Rc<String>,
        pub hash: [u8; 32],
    }

    impl Downloadable for LocalMedia {
        fn direct_path(&self) -> Option<&str> {
            Some(&self.path)
        }
        fn media_key(&self) -> Option<&[u8]> {
            None
        }
        fn file_enc_sha256(&self) -> Option<&[u8]> {
            None
        }
        fn file_sha256(&self) -> Option<&[u8]> {
            Some(&self.hash)
        }
        fn file_length(&self) -> Option<u64> {
            None
        }
        fn app_info(&self) -> MediaType {
            MediaType::Image
        }
    }

    pub struct LocalWriter(pub Rc<RefCell<Cursor<Vec<u8>>>>);

    impl Write for LocalWriter {
        fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
            self.0.borrow_mut().write(bytes)
        }
        fn flush(&mut self) -> io::Result<()> {
            self.0.borrow_mut().flush()
        }
    }
    impl Seek for LocalWriter {
        fn seek(&mut self, pos: SeekFrom) -> io::Result<u64> {
            self.0.borrow_mut().seek(pos)
        }
    }
    impl DownloadWriter for LocalWriter {
        fn truncate(&mut self, len: u64) -> io::Result<()> {
            self.0
                .borrow_mut()
                .get_mut()
                .truncate(usize::try_from(len).unwrap_or(usize::MAX));
            Ok(())
        }
    }

    fn local(_: impl Future) {}

    pub fn browser_futures(client: &Client, downloader: &MediaDownloader, media: &LocalMedia) {
        let sink = Rc::new(RefCell::new(Cursor::new(Vec::new())));
        local(client.download(media));
        local(client.download_to_writer(media, LocalWriter(sink.clone())));
        local(downloader.download(media));
        local(downloader.download_to_writer(media, LocalWriter(sink)));
    }
}

#[cfg(target_arch = "wasm32")]
pub use browser::*;

/// Native download sinks must be movable to the blocking pool.
///
/// ```compile_fail,E0277
/// use std::{io::{self, Cursor, Seek, SeekFrom, Write}, rc::Rc};
/// use whatsapp_rust::download::{Downloadable, DownloadWriter, MediaDownloader};
/// struct LocalSink(Cursor<Vec<u8>>, Rc<()>);
/// impl Write for LocalSink {
///     fn write(&mut self, bytes: &[u8]) -> io::Result<usize> { self.0.write(bytes) }
///     fn flush(&mut self) -> io::Result<()> { self.0.flush() }
/// }
/// impl Seek for LocalSink {
///     fn seek(&mut self, pos: SeekFrom) -> io::Result<u64> { self.0.seek(pos) }
/// }
/// impl DownloadWriter for LocalSink {
///     fn truncate(&mut self, len: u64) -> io::Result<()> {
///         self.0.get_mut().truncate(len as usize);
///         Ok(())
///     }
/// }
/// fn rejected(downloader: &MediaDownloader, media: &dyn Downloadable) {
///     let _ = downloader.download_to_writer(media, LocalSink(Cursor::new(vec![]), Rc::new(())));
/// }
/// ```
///
/// Native references must also remain shareable across tasks.
///
/// ```compile_fail,E0277
/// use std::rc::Rc;
/// use whatsapp_rust::download::{Downloadable, MediaType};
/// struct LocalMedia(Rc<()>);
/// impl Downloadable for LocalMedia {
///     fn direct_path(&self) -> Option<&str> { Some("/file") }
///     fn media_key(&self) -> Option<&[u8]> { None }
///     fn file_enc_sha256(&self) -> Option<&[u8]> { None }
///     fn file_sha256(&self) -> Option<&[u8]> { None }
///     fn file_length(&self) -> Option<u64> { None }
///     fn app_info(&self) -> MediaType { MediaType::Image }
/// }
/// ```
#[cfg(not(target_arch = "wasm32"))]
pub struct NativeBounds;

/// Validated metadata construction works on native and WASM hosts.
/// Callers inspect metadata through the read-only Downloadable trait.
pub fn validated_params() -> Result<(), whatsapp_rust::download::DownloadPreparationError> {
    use whatsapp_rust::download::{DownloadParams, MediaType};
    let encrypted = DownloadParams::encrypted(
        "/file",
        &[1; 32],
        &[2; 32],
        &[3; 32],
        16,
        MediaType::MusicArtwork,
    )?;
    assert_eq!(encrypted.media_key(), Some(&[1; 32][..]));
    let plaintext =
        DownloadParams::plaintext("/file", &[2; 32], 16, MediaType::NewsletterMusicArtwork)?;
    assert!(plaintext.media_key().is_none());
    assert!(!plaintext.is_encrypted());
    Ok(())
}

/// DownloadParams cannot be invalidated after construction.
///
/// ```compile_fail,E0616
/// use whatsapp_rust::download::{DownloadParams, MediaType};
/// let mut params = DownloadParams::encrypted("/file", &[1; 32], &[2; 32], &[3; 32], 16, MediaType::Image).unwrap();
/// params.media_key = None;
/// ```
///
/// Raw struct literals are no longer a second construction interface.
///
/// ```compile_fail,E0451
/// use whatsapp_rust::download::{DownloadParams, MediaType};
/// let params = DownloadParams {
///     direct_path: "/file".into(), media_key: None,
///     file_sha256: vec![2; 32], file_enc_sha256: None,
///     file_length: 16, media_type: MediaType::Image,
/// };
/// ```
pub struct ValidatedMetadata;
