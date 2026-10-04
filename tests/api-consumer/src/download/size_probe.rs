fn main() {
    // Retain the public URL preparation path without network activity. A real
    // function pointer keeps its linked code/data in native and WASM binaries.
    let prepare = wacore::download::DownloadUtils::prepare_download_requests as *const ();
    std::hint::black_box(prepare);
}
