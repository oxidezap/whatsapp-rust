use whatsapp_rust::store::SqliteStore;
use whatsapp_rust::store::traits::DeviceStore;

// Real SQL through both adapters proves linking, migrations, and runtime host
// hooks. All records are synthetic and both databases live only in memory.
#[cfg_attr(not(target_family = "wasm"), tokio::test)]
#[cfg_attr(target_family = "wasm", wasm_bindgen_test::wasm_bindgen_test)]
async fn session_store_and_archive_work_in_one_process() {
    let archive = rusqlite::Connection::open_in_memory().unwrap();
    archive
        .execute_batch("CREATE TABLE archive (body TEXT NOT NULL);")
        .unwrap();
    archive
        .execute("INSERT INTO archive VALUES (?1)", ["synthetic message"])
        .unwrap();
    let body: String = archive
        .query_row("SELECT body FROM archive", [], |row| row.get(0))
        .unwrap();
    assert_eq!(body, "synthetic message");

    // No key is set. This verifies the SQLCipher build and plaintext operation,
    // without claiming to test encryption or an external application's archive.
    #[cfg(all(feature = "sqlcipher", not(target_family = "wasm")))]
    {
        let version: String = archive
            .query_row("PRAGMA cipher_version", [], |row| row.get(0))
            .unwrap();
        assert!(!version.is_empty());
    }

    let store = SqliteStore::open("file:sqlite_interop?mode=memory&cache=shared")
        .await
        .unwrap();
    let db = store.database();
    assert!(!store.exists().await.unwrap());
    db.provision_device(store.device_id()).await.unwrap();
    assert!(store.exists().await.unwrap());
    let second = db.create_device().await.unwrap();
    assert_ne!(store.device_id(), second.device_id());
    assert_eq!(db.list_devices().await.unwrap().len(), 2);
    assert!(second.exists().await.unwrap());
    // Interleave another archive query after Diesel's migrations and writes.
    assert_eq!(
        archive
            .query_row("SELECT count(*) FROM archive", [], |r| r.get::<_, i64>(0))
            .unwrap(),
        1
    );
}
