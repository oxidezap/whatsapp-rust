use whatsapp_rust::store::SqliteStore;
use whatsapp_rust::store::traits::DeviceStore;

// Export the same operation for local browser harnesses and the registered
// browser test. A real open retains SQLite's host imports through final linking.
#[cfg_attr(target_family = "wasm", wasm_bindgen::prelude::wasm_bindgen)]
pub async fn probe() -> Result<String, String> {
    let store = SqliteStore::open("file:sdk_browser_probe?mode=memory&cache=shared")
        .await
        .map_err(|error| error.to_string())?;
    let db = store.database();
    db.provision_device(store.device_id())
        .await
        .map_err(|error| error.to_string())?;
    let second = db
        .create_device()
        .await
        .map_err(|error| error.to_string())?;
    let devices = db.list_devices().await.map_err(|error| error.to_string())?;
    if devices.len() != 2
        || store.device_id() == second.device_id()
        || !store.exists().await.map_err(|error| error.to_string())?
        || !second.exists().await.map_err(|error| error.to_string())?
    {
        return Err("SDK account provisioning did not persist both devices".into());
    }
    Ok("migrations + provision + exists passed".into())
}
