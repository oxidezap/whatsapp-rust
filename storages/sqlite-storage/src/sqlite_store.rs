use crate::schema::*;
use async_trait::async_trait;
use bytes::Bytes;
use diesel::prelude::*;
use diesel::r2d2::{ConnectionManager, Pool};
use diesel::result::{DatabaseErrorKind, Error as DieselError};
use diesel::sqlite::SqliteConnection;
use diesel::upsert::excluded;
use diesel_migrations::{EmbeddedMigrations, MigrationHarness, embed_migrations};
use log::warn;
use std::sync::Arc;
use wacore::appstate::hash::HashState;
use wacore::appstate::processor::AppStateMutationMAC;
use wacore::libsignal::protocol::{KeyPair, PrivateKey, PublicKey};
use wacore::store::Device as CoreDevice;
use wacore::store::error::{Result, StoreError};
use wacore::store::traits::*;

/// Internal error type that preserves the Diesel error for structured matching
/// before converting to `StoreError`. Used in retry loops where we need to
/// distinguish retriable SQLite lock errors from other failures.
enum DieselOrStore {
    Diesel(DieselError),
    Store(StoreError),
}

impl From<DieselOrStore> for StoreError {
    fn from(e: DieselOrStore) -> Self {
        match e {
            DieselOrStore::Diesel(e) => StoreError::Database(Box::new(e)),
            DieselOrStore::Store(e) => e,
        }
    }
}

/// Check if a Diesel error represents a retriable SQLite lock contention.
///
/// SQLite BUSY (error code 5) and LOCKED (error code 6) both map to
/// `DatabaseError(Unknown, _)` in Diesel. We inspect the error message
/// from `sqlite3_errmsg()` to distinguish them from other unknown errors.
fn is_retriable_sqlite_error(error: &DieselError) -> bool {
    match error {
        DieselError::DatabaseError(DatabaseErrorKind::Unknown, info) => {
            let msg = info.message();
            msg.contains("locked") || msg.contains("busy")
        }
        _ => false,
    }
}

const MIGRATIONS: EmbeddedMigrations = embed_migrations!("migrations");

type SqlitePool = Pool<ConnectionManager<SqliteConnection>>;

/// Row representation for the `device` table.
///
/// Field order must match the column order in `schema::device`.
/// Using a named struct instead of a positional tuple so fields are
/// accessed by name, reducing the risk of mix-ups when columns are added.
#[derive(Queryable, Selectable)]
#[diesel(table_name = crate::schema::device)]
#[allow(dead_code)]
struct DeviceRow {
    id: i32,
    lid: String,
    pn: String,
    registration_id: i32,
    noise_key: Vec<u8>,
    identity_key: Vec<u8>,
    signed_pre_key: Vec<u8>,
    signed_pre_key_id: i32,
    signed_pre_key_signature: Vec<u8>,
    adv_secret_key: Vec<u8>,
    account: Option<Vec<u8>>,
    push_name: String,
    app_version_primary: i32,
    app_version_secondary: i32,
    app_version_tertiary: i64,
    app_version_last_fetched_ms: i64,
    edge_routing_info: Option<Vec<u8>>,
    props_hash: Option<String>,
    next_pre_key_id: i32,
    nct_salt: Option<Vec<u8>>,
    server_has_prekeys: bool,
    server_cert_chain: Option<Vec<u8>>,
    login_counter: i32,
    first_unupload_pre_key_id: i32,
}

/// Max ids per `eq_any` list, under SQLite's default 999 host-parameter limit.
const ID_PARAM_CHUNK: usize = 900;

#[derive(Clone)]
pub struct SqliteStore {
    pub(crate) pool: SqlitePool,
    pub(crate) db_semaphore: Arc<tokio::sync::Semaphore>,
    pub(crate) database_path: String,
    device_id: i32,
}

#[derive(Debug, Clone, Copy)]
struct ConnectionOptions;

impl diesel::r2d2::CustomizeConnection<SqliteConnection, diesel::r2d2::Error>
    for ConnectionOptions
{
    fn on_acquire(
        &self,
        conn: &mut SqliteConnection,
    ) -> std::result::Result<(), diesel::r2d2::Error> {
        diesel::sql_query("PRAGMA busy_timeout = 30000;")
            .execute(conn)
            .map_err(diesel::r2d2::Error::QueryError)?;
        diesel::sql_query("PRAGMA synchronous = NORMAL;")
            .execute(conn)
            .map_err(diesel::r2d2::Error::QueryError)?;
        diesel::sql_query("PRAGMA cache_size = 512;")
            .execute(conn)
            .map_err(diesel::r2d2::Error::QueryError)?;
        diesel::sql_query("PRAGMA temp_store = memory;")
            .execute(conn)
            .map_err(diesel::r2d2::Error::QueryError)?;
        diesel::sql_query("PRAGMA foreign_keys = ON;")
            .execute(conn)
            .map_err(diesel::r2d2::Error::QueryError)?;
        Ok(())
    }
}

fn parse_database_path(database_url: &str) -> Result<String> {
    // Reject in-memory databases
    if database_url == ":memory:" {
        return Err(StoreError::InvalidConfig(
            "Snapshot not supported for in-memory databases".to_string(),
        ));
    }

    // Strip query string and fragment
    let path = database_url
        .split(['?', '#'])
        .next()
        .unwrap_or(database_url);

    // Remove sqlite:// prefix if present
    let path = path.trim_start_matches("sqlite://");

    // Check if the resulting path looks like an in-memory marker
    if path == ":memory:" || path.starts_with(":memory:?") {
        return Err(StoreError::InvalidConfig(
            "Snapshot not supported for in-memory databases".to_string(),
        ));
    }

    Ok(path.to_string())
}

impl SqliteStore {
    pub async fn new(database_url: &str) -> std::result::Result<Self, StoreError> {
        let manager = ConnectionManager::<SqliteConnection>::new(database_url);

        let pool_size = 2;

        // Local SQLite file connections don't spontaneously drop, so r2d2's
        // default per-checkout liveness probe (a SELECT 1 via Diesel's
        // is_valid) is pure overhead on every store op: any real failure
        // surfaces as a StoreError on the next actual query, so the probe
        // guards nothing here. Skipping it saves a cached SELECT 1 (reset+step)
        // per pool.get().
        let pool = Pool::builder()
            .max_size(pool_size)
            .test_on_check_out(false)
            .connection_customizer(Box::new(ConnectionOptions))
            .build(manager)
            .map_err(|e| StoreError::Connection(Box::new(e)))?;

        let pool_clone = pool.clone();
        tokio::task::spawn_blocking(move || -> std::result::Result<(), StoreError> {
            let mut conn = pool_clone
                .get()
                .map_err(|e| StoreError::Connection(Box::new(e)))?;

            diesel::sql_query("PRAGMA journal_mode = WAL;")
                .execute(&mut conn)
                .map_err(|e| StoreError::Database(Box::new(e)))?;

            conn.run_pending_migrations(MIGRATIONS)
                .map_err(StoreError::Migration)?;

            Ok(())
        })
        .await
        .map_err(|e| StoreError::Database(Box::new(e)))??;

        let database_path = parse_database_path(database_url)?;

        Ok(Self {
            pool,
            db_semaphore: Arc::new(tokio::sync::Semaphore::new(1)),
            database_path,
            device_id: 1,
        })
    }

    pub async fn new_for_device(
        database_url: &str,
        device_id: i32,
    ) -> std::result::Result<Self, StoreError> {
        let mut store = Self::new(database_url).await?;
        store.device_id = device_id;
        Ok(store)
    }

    pub fn device_id(&self) -> i32 {
        self.device_id
    }

    async fn with_semaphore<F, T>(&self, f: F) -> Result<T>
    where
        F: FnOnce() -> Result<T> + Send + 'static,
        T: Send + 'static,
    {
        let permit = self
            .db_semaphore
            .clone()
            .acquire_owned()
            .await
            .map_err(|e| StoreError::Database(Box::new(e)))?;
        let result = tokio::task::spawn_blocking(move || {
            let res = f();
            drop(permit);
            res
        })
        .await
        .map_err(|e| StoreError::Database(Box::new(e)))??;
        Ok(result)
    }

    /// Execute a database operation with semaphore serialization and retry on
    /// transient SQLite lock/busy errors. Mirrors WhatsApp Web's PromiseQueue
    /// pattern that serializes database commits to avoid concurrent write contention.
    async fn with_retry<F, T>(&self, op_name: &str, make_op: F) -> Result<T>
    where
        F: Fn() -> Box<
            dyn FnOnce(&mut SqliteConnection) -> std::result::Result<T, DieselError> + Send,
        >,
        T: Send + 'static,
    {
        const MAX_RETRIES: u32 = 5;

        for attempt in 0..=MAX_RETRIES {
            let permit = self
                .db_semaphore
                .clone()
                .acquire_owned()
                .await
                .map_err(|e| StoreError::Database(Box::new(e)))?;

            let pool = self.pool.clone();
            let op = make_op();

            let result =
                tokio::task::spawn_blocking(move || -> std::result::Result<T, DieselOrStore> {
                    let _permit = permit;
                    let mut conn = pool
                        .get()
                        .map_err(|e| DieselOrStore::Store(StoreError::Connection(Box::new(e))))?;
                    op(&mut conn).map_err(DieselOrStore::Diesel)
                })
                .await;

            match result {
                Ok(Ok(val)) => return Ok(val),
                Ok(Err(DieselOrStore::Diesel(ref e)))
                    if is_retriable_sqlite_error(e) && attempt < MAX_RETRIES =>
                {
                    let delay_ms = 10u64 * (1u64 << attempt.min(4));
                    // Skip the first transient blip; warn from the second retry on so
                    // sustained busy/locked contention doesn't go unobserved.
                    if attempt >= 1 {
                        warn!(
                            "{op_name} busy/locked, retry {}/{} in {delay_ms}ms: {e}",
                            attempt + 1,
                            MAX_RETRIES + 1
                        );
                    }
                    tokio::time::sleep(tokio::time::Duration::from_millis(delay_ms)).await;
                }
                Ok(Err(e)) => return Err(e.into()),
                Err(e) => return Err(StoreError::Database(Box::new(e))),
            }
        }

        Err(StoreError::RetriesExhausted {
            op: op_name.to_string(),
        })
    }

    fn serialize_keypair(&self, key_pair: &KeyPair) -> Result<Vec<u8>> {
        let mut bytes = Vec::with_capacity(64);
        bytes.extend_from_slice(key_pair.private_key.serialize());
        bytes.extend_from_slice(key_pair.public_key.public_key_bytes());
        Ok(bytes)
    }

    fn deserialize_keypair(&self, bytes: &[u8]) -> Result<KeyPair> {
        if bytes.len() != 64 {
            return Err(StoreError::Validation(format!(
                "Invalid KeyPair length: {}",
                bytes.len()
            )));
        }

        let private_key = PrivateKey::deserialize(&bytes[0..32])
            .map_err(|e| StoreError::Serialization(Box::new(e)))?;
        let public_key = PublicKey::from_djb_public_key_bytes(&bytes[32..64])
            .map_err(|e| StoreError::Serialization(Box::new(e)))?;

        Ok(KeyPair::new(public_key, private_key))
    }

    pub async fn save_device_data_for_device(
        &self,
        device_id: i32,
        device_data: &CoreDevice,
    ) -> Result<()> {
        // Use Arc so retry clones are just atomic increments, not deep copies.
        let noise_key_data: Arc<[u8]> = self.serialize_keypair(&device_data.noise_key)?.into();
        let identity_key_data: Arc<[u8]> =
            self.serialize_keypair(&device_data.identity_key)?.into();
        let signed_pre_key_data: Arc<[u8]> =
            self.serialize_keypair(&device_data.signed_pre_key)?.into();
        let account_data: Option<Arc<[u8]>> = device_data
            .account
            .as_ref()
            .map(|a| Arc::from(wacore::store::device::account_serde::to_bytes(a)));
        let registration_id = device_data.registration_id as i32;
        let signed_pre_key_id = device_data.signed_pre_key_id as i32;
        let signed_pre_key_signature: Arc<[u8]> =
            Arc::from(&device_data.signed_pre_key_signature[..]);
        let adv_secret_key: Arc<[u8]> = Arc::from(&device_data.adv_secret_key[..]);
        let push_name: Arc<str> = Arc::from(device_data.push_name.as_str());
        let app_version_primary = device_data.app_version_primary as i32;
        let app_version_secondary = device_data.app_version_secondary as i32;
        let app_version_tertiary = device_data.app_version_tertiary as i64;
        let app_version_last_fetched_ms = device_data.app_version_last_fetched_ms;
        let edge_routing_info: Option<Arc<[u8]>> =
            device_data.edge_routing_info.as_deref().map(Arc::from);
        let props_hash: Option<Arc<str>> = device_data.props_hash.as_deref().map(Arc::from);
        let next_pre_key_id = device_data.next_pre_key_id as i32;
        let first_unupload_pre_key_id = device_data.first_unupload_pre_key_id as i32;
        let server_has_prekeys = device_data.server_has_prekeys;
        let nct_salt: Option<Arc<[u8]>> = device_data.nct_salt.as_deref().map(Arc::from);
        let server_cert_chain: Option<Arc<[u8]>> = device_data
            .server_cert_chain
            .as_ref()
            .map(|chain| Arc::from(crate::wire::encode_server_cert_chain(chain)));
        let login_counter = device_data.login_counter;
        let new_lid: Arc<str> = Arc::from(
            device_data
                .lid
                .as_ref()
                .map(|j| j.to_string())
                .unwrap_or_default()
                .as_str(),
        );
        let new_pn: Arc<str> = Arc::from(
            device_data
                .pn
                .as_ref()
                .map(|j| j.to_string())
                .unwrap_or_default()
                .as_str(),
        );

        self.with_retry("save_device_data", || {
            let noise_key_data = Arc::clone(&noise_key_data);
            let identity_key_data = Arc::clone(&identity_key_data);
            let signed_pre_key_data = Arc::clone(&signed_pre_key_data);
            let account_data = account_data.clone();
            let signed_pre_key_signature = Arc::clone(&signed_pre_key_signature);
            let adv_secret_key = Arc::clone(&adv_secret_key);
            let push_name = Arc::clone(&push_name);
            let edge_routing_info = edge_routing_info.clone();
            let props_hash = props_hash.clone();
            let nct_salt = nct_salt.clone();
            let server_cert_chain = server_cert_chain.clone();
            let new_lid = Arc::clone(&new_lid);
            let new_pn = Arc::clone(&new_pn);

            Box::new(move |conn: &mut SqliteConnection| {
                diesel::insert_into(device::table)
                    .values((
                        device::id.eq(device_id),
                        device::lid.eq(&*new_lid),
                        device::pn.eq(&*new_pn),
                        device::registration_id.eq(registration_id),
                        device::noise_key.eq(&*noise_key_data),
                        device::identity_key.eq(&*identity_key_data),
                        device::signed_pre_key.eq(&*signed_pre_key_data),
                        device::signed_pre_key_id.eq(signed_pre_key_id),
                        device::signed_pre_key_signature.eq(&*signed_pre_key_signature),
                        device::adv_secret_key.eq(&*adv_secret_key),
                        device::account.eq(account_data.as_deref()),
                        device::push_name.eq(&*push_name),
                        device::app_version_primary.eq(app_version_primary),
                        device::app_version_secondary.eq(app_version_secondary),
                        device::app_version_tertiary.eq(app_version_tertiary),
                        device::app_version_last_fetched_ms.eq(app_version_last_fetched_ms),
                        device::edge_routing_info.eq(edge_routing_info.as_deref()),
                        device::props_hash.eq(props_hash.as_deref()),
                        device::next_pre_key_id.eq(next_pre_key_id),
                        device::first_unupload_pre_key_id.eq(first_unupload_pre_key_id),
                        device::server_has_prekeys.eq(server_has_prekeys),
                        device::nct_salt.eq(nct_salt.as_deref()),
                        device::server_cert_chain.eq(server_cert_chain.as_deref()),
                        device::login_counter.eq(login_counter),
                    ))
                    .on_conflict(device::id)
                    .do_update()
                    .set((
                        device::lid.eq(excluded(device::lid)),
                        device::pn.eq(excluded(device::pn)),
                        device::registration_id.eq(excluded(device::registration_id)),
                        device::noise_key.eq(excluded(device::noise_key)),
                        device::identity_key.eq(excluded(device::identity_key)),
                        device::signed_pre_key.eq(excluded(device::signed_pre_key)),
                        device::signed_pre_key_id.eq(excluded(device::signed_pre_key_id)),
                        device::signed_pre_key_signature
                            .eq(excluded(device::signed_pre_key_signature)),
                        device::adv_secret_key.eq(excluded(device::adv_secret_key)),
                        device::account.eq(excluded(device::account)),
                        device::push_name.eq(excluded(device::push_name)),
                        device::app_version_primary.eq(excluded(device::app_version_primary)),
                        device::app_version_secondary.eq(excluded(device::app_version_secondary)),
                        device::app_version_tertiary.eq(excluded(device::app_version_tertiary)),
                        device::app_version_last_fetched_ms
                            .eq(excluded(device::app_version_last_fetched_ms)),
                        device::edge_routing_info.eq(excluded(device::edge_routing_info)),
                        device::props_hash.eq(excluded(device::props_hash)),
                        device::next_pre_key_id.eq(excluded(device::next_pre_key_id)),
                        device::first_unupload_pre_key_id
                            .eq(excluded(device::first_unupload_pre_key_id)),
                        device::server_has_prekeys.eq(excluded(device::server_has_prekeys)),
                        device::nct_salt.eq(excluded(device::nct_salt)),
                        device::server_cert_chain.eq(excluded(device::server_cert_chain)),
                        device::login_counter.eq(excluded(device::login_counter)),
                    ))
                    .execute(conn)
                    .map(|_| ())
            })
        })
        .await
    }

    pub async fn create_new_device(&self) -> Result<i32> {
        let device_id = self.device_id;
        let new_device = wacore::store::Device::new();

        let noise_key_data: Arc<[u8]> = self.serialize_keypair(&new_device.noise_key)?.into();
        let identity_key_data: Arc<[u8]> = self.serialize_keypair(&new_device.identity_key)?.into();
        let signed_pre_key_data: Arc<[u8]> =
            self.serialize_keypair(&new_device.signed_pre_key)?.into();
        let registration_id = new_device.registration_id as i32;
        let signed_pre_key_id = new_device.signed_pre_key_id as i32;
        let signed_pre_key_signature: Arc<[u8]> =
            Arc::from(&new_device.signed_pre_key_signature[..]);
        let adv_secret_key: Arc<[u8]> = Arc::from(&new_device.adv_secret_key[..]);
        let push_name: Arc<str> = Arc::from(new_device.push_name.as_str());
        let app_version_primary = new_device.app_version_primary as i32;
        let app_version_secondary = new_device.app_version_secondary as i32;
        let app_version_tertiary = new_device.app_version_tertiary as i64;
        let app_version_last_fetched_ms = new_device.app_version_last_fetched_ms;
        let next_pre_key_id = new_device.next_pre_key_id as i32;
        let first_unupload_pre_key_id = new_device.first_unupload_pre_key_id as i32;
        let server_has_prekeys = new_device.server_has_prekeys;

        self.with_retry("create_new_device", || {
            let noise_key_data = Arc::clone(&noise_key_data);
            let identity_key_data = Arc::clone(&identity_key_data);
            let signed_pre_key_data = Arc::clone(&signed_pre_key_data);
            let signed_pre_key_signature = Arc::clone(&signed_pre_key_signature);
            let adv_secret_key = Arc::clone(&adv_secret_key);
            let push_name = Arc::clone(&push_name);

            Box::new(move |conn: &mut SqliteConnection| {
                diesel::insert_into(device::table)
                    .values((
                        device::id.eq(device_id),
                        device::lid.eq(""),
                        device::pn.eq(""),
                        device::registration_id.eq(registration_id),
                        device::noise_key.eq(&*noise_key_data),
                        device::identity_key.eq(&*identity_key_data),
                        device::signed_pre_key.eq(&*signed_pre_key_data),
                        device::signed_pre_key_id.eq(signed_pre_key_id),
                        device::signed_pre_key_signature.eq(&*signed_pre_key_signature),
                        device::adv_secret_key.eq(&*adv_secret_key),
                        device::account.eq(None::<&[u8]>),
                        device::push_name.eq(&*push_name),
                        device::app_version_primary.eq(app_version_primary),
                        device::app_version_secondary.eq(app_version_secondary),
                        device::app_version_tertiary.eq(app_version_tertiary),
                        device::app_version_last_fetched_ms.eq(app_version_last_fetched_ms),
                        device::edge_routing_info.eq(None::<&[u8]>),
                        device::props_hash.eq(None::<&str>),
                        device::next_pre_key_id.eq(next_pre_key_id),
                        device::first_unupload_pre_key_id.eq(first_unupload_pre_key_id),
                        device::server_has_prekeys.eq(server_has_prekeys),
                        device::nct_salt.eq(None::<&[u8]>),
                        device::server_cert_chain.eq(None::<&[u8]>),
                        device::login_counter.eq(0i32),
                    ))
                    .execute(conn)
                    .map(|_| device_id)
            })
        })
        .await
    }

    pub async fn device_exists(&self, device_id: i32) -> Result<bool> {
        use crate::schema::device;

        let pool = self.pool.clone();
        tokio::task::spawn_blocking(move || -> Result<bool> {
            let mut conn = pool
                .get()
                .map_err(|e| StoreError::Connection(Box::new(e)))?;

            let count: i64 = device::table
                .filter(device::id.eq(device_id))
                .count()
                .get_result(&mut conn)
                .map_err(|e| StoreError::Database(Box::new(e)))?;

            Ok(count > 0)
        })
        .await
        .map_err(|e| StoreError::Database(Box::new(e)))?
    }

    pub async fn load_device_data_for_device(&self, device_id: i32) -> Result<Option<CoreDevice>> {
        use crate::schema::device;

        let pool = self.pool.clone();
        let row = tokio::task::spawn_blocking(move || -> Result<Option<DeviceRow>> {
            let mut conn = pool
                .get()
                .map_err(|e| StoreError::Connection(Box::new(e)))?;
            let result = device::table
                .filter(device::id.eq(device_id))
                .first::<DeviceRow>(&mut conn)
                .optional()
                .map_err(|e| StoreError::Database(Box::new(e)))?;
            Ok(result)
        })
        .await
        .map_err(|e| StoreError::Database(Box::new(e)))??;

        if let Some(row) = row {
            let pn = if !row.pn.is_empty() {
                row.pn.parse().ok()
            } else {
                None
            };
            let lid = if !row.lid.is_empty() {
                row.lid.parse().ok()
            } else {
                None
            };

            let noise_key = self.deserialize_keypair(&row.noise_key)?;
            let identity_key = self.deserialize_keypair(&row.identity_key)?;
            let signed_pre_key = self.deserialize_keypair(&row.signed_pre_key)?;

            let signed_pre_key_signature: [u8; 64] =
                row.signed_pre_key_signature.try_into().map_err(|_| {
                    StoreError::Validation("Invalid signed_pre_key_signature length".to_string())
                })?;

            let adv_secret_key: [u8; 32] = row
                .adv_secret_key
                .try_into()
                .map_err(|_| StoreError::Validation("Invalid adv_secret_key length".to_string()))?;

            let account = row
                .account
                .map(|data| {
                    wacore::store::device::account_serde::from_bytes(&data)
                        .map_err(|e| StoreError::Serialization(Box::new(e)))
                })
                .transpose()?;

            Ok(Some(CoreDevice {
                pn,
                lid,
                registration_id: row.registration_id as u32,
                noise_key,
                identity_key,
                signed_pre_key,
                signed_pre_key_id: row.signed_pre_key_id as u32,
                signed_pre_key_signature,
                adv_secret_key,
                account: account.map(std::sync::Arc::new),
                push_name: row.push_name,
                app_version_primary: row.app_version_primary as u32,
                app_version_secondary: row.app_version_secondary as u32,
                app_version_tertiary: row.app_version_tertiary.try_into().unwrap_or(0u32),
                app_version_last_fetched_ms: row.app_version_last_fetched_ms,
                device_props: std::sync::Arc::new(wacore::store::device::DEVICE_PROPS.clone()),
                client_profile: wacore::client_profile::ClientProfile::web(),
                edge_routing_info: row.edge_routing_info,
                props_hash: row.props_hash,
                next_pre_key_id: row.next_pre_key_id as u32,
                first_unupload_pre_key_id: row.first_unupload_pre_key_id as u32,
                server_has_prekeys: row.server_has_prekeys,
                nct_salt: row.nct_salt,
                nct_salt_sync_seen: false,
                server_cert_chain: row
                    .server_cert_chain
                    .as_deref()
                    .and_then(|bytes| {
                        // The cert chain is a perf cache, not load-bearing
                        // identity. A corrupt blob (truncated row, format
                        // change between versions) must NOT block startup —
                        // log it and degrade to None so the next connect
                        // simply pays one XX handshake to repopulate.
                        match crate::wire::decode_server_cert_chain(bytes) {
                            Ok(chain) => Some(chain),
                            Err(e) => {
                                log::warn!(
                                    "device {} server_cert_chain blob ({} bytes) failed to decode: {e}; \
                                     dropping cache, next connect will use XX",
                                    self.device_id,
                                    bytes.len(),
                                );
                                None
                            }
                        }
                    }),
                login_counter: row.login_counter,
            }))
        } else {
            Ok(None)
        }
    }

    pub async fn put_identity_for_device(
        &self,
        address: &str,
        key: [u8; 32],
        device_id: i32,
    ) -> Result<()> {
        let pool = self.pool.clone();
        let db_semaphore = self.db_semaphore.clone();
        let address_owned = address.to_string();
        let key_vec = key.to_vec();

        const MAX_RETRIES: u32 = 5;

        for attempt in 0..=MAX_RETRIES {
            let permit = db_semaphore
                .clone()
                .acquire_owned()
                .await
                .map_err(|e| StoreError::Database(Box::new(e)))?;

            let pool_clone = pool.clone();
            let address_clone = address_owned.clone();
            let key_clone = key_vec.clone();

            let result =
                tokio::task::spawn_blocking(move || -> std::result::Result<(), DieselOrStore> {
                    let mut conn = pool_clone
                        .get()
                        .map_err(|e| DieselOrStore::Store(StoreError::Connection(Box::new(e))))?;
                    diesel::insert_into(identities::table)
                        .values((
                            identities::address.eq(address_clone),
                            identities::key.eq(&key_clone[..]),
                            identities::device_id.eq(device_id),
                        ))
                        .on_conflict((identities::address, identities::device_id))
                        .do_update()
                        .set(identities::key.eq(&key_clone[..]))
                        .execute(&mut conn)
                        .map_err(DieselOrStore::Diesel)?;
                    Ok(())
                })
                .await;

            drop(permit);

            match result {
                Ok(Ok(())) => return Ok(()),
                Ok(Err(DieselOrStore::Diesel(ref e)))
                    if is_retriable_sqlite_error(e) && attempt < MAX_RETRIES =>
                {
                    let delay_ms = 10 * 2u64.pow(attempt);
                    warn!(
                        "Identity write failed (attempt {}/{}): {e}. Retrying in {delay_ms}ms...",
                        attempt + 1,
                        MAX_RETRIES + 1,
                    );
                    tokio::time::sleep(std::time::Duration::from_millis(delay_ms)).await;
                    continue;
                }
                Ok(Err(e)) => return Err(e.into()),
                Err(e) => return Err(StoreError::Database(Box::new(e))),
            }
        }

        Err(StoreError::RetriesExhausted {
            op: format!("identity_write (after {} attempts)", MAX_RETRIES + 1),
        })
    }

    pub async fn delete_identity_for_device(&self, address: &str, device_id: i32) -> Result<()> {
        let pool = self.pool.clone();
        let address_owned = address.to_string();

        tokio::task::spawn_blocking(move || -> Result<()> {
            let mut conn = pool
                .get()
                .map_err(|e| StoreError::Connection(Box::new(e)))?;
            diesel::delete(
                identities::table
                    .filter(identities::address.eq(address_owned))
                    .filter(identities::device_id.eq(device_id)),
            )
            .execute(&mut conn)
            .map_err(|e| StoreError::Database(Box::new(e)))?;
            Ok(())
        })
        .await
        .map_err(|e| StoreError::Database(Box::new(e)))??;

        Ok(())
    }

    pub async fn load_identity_for_device(
        &self,
        address: &str,
        device_id: i32,
    ) -> Result<Option<Vec<u8>>> {
        let pool = self.pool.clone();
        let address = address.to_string();
        let result = self
            .with_semaphore(move || -> Result<Option<Vec<u8>>> {
                let mut conn = pool
                    .get()
                    .map_err(|e| StoreError::Connection(Box::new(e)))?;
                let res: Option<Vec<u8>> = identities::table
                    .select(identities::key)
                    .filter(identities::address.eq(address))
                    .filter(identities::device_id.eq(device_id))
                    .first(&mut conn)
                    .optional()
                    .map_err(|e| StoreError::Database(Box::new(e)))?;
                Ok(res)
            })
            .await?;

        Ok(result)
    }

    pub async fn get_session_for_device(
        &self,
        address: &str,
        device_id: i32,
    ) -> Result<Option<Vec<u8>>> {
        let pool = self.pool.clone();
        let address_for_query = address.to_string();
        let result = self
            .with_semaphore(move || -> Result<Option<Vec<u8>>> {
                let mut conn = pool
                    .get()
                    .map_err(|e| StoreError::Connection(Box::new(e)))?;
                let res: Option<Vec<u8>> = sessions::table
                    .select(sessions::record)
                    .filter(sessions::address.eq(address_for_query.clone()))
                    .filter(sessions::device_id.eq(device_id))
                    .first(&mut conn)
                    .optional()
                    .map_err(|e| StoreError::Database(Box::new(e)))?;

                Ok(res)
            })
            .await?;

        Ok(result)
    }

    pub async fn put_session_for_device(
        &self,
        address: &str,
        session: &[u8],
        device_id: i32,
    ) -> Result<()> {
        let pool = self.pool.clone();
        let db_semaphore = self.db_semaphore.clone();
        let address_owned = address.to_string();
        let session_vec = session.to_vec();

        const MAX_RETRIES: u32 = 5;

        for attempt in 0..=MAX_RETRIES {
            let permit = db_semaphore
                .clone()
                .acquire_owned()
                .await
                .map_err(|e| StoreError::Database(Box::new(e)))?;

            let pool_clone = pool.clone();
            let address_clone = address_owned.clone();
            let session_clone = session_vec.clone();

            let result =
                tokio::task::spawn_blocking(move || -> std::result::Result<(), DieselOrStore> {
                    let mut conn = pool_clone
                        .get()
                        .map_err(|e| DieselOrStore::Store(StoreError::Connection(Box::new(e))))?;
                    diesel::insert_into(sessions::table)
                        .values((
                            sessions::address.eq(address_clone),
                            sessions::record.eq(&session_clone),
                            sessions::device_id.eq(device_id),
                        ))
                        .on_conflict((sessions::address, sessions::device_id))
                        .do_update()
                        .set(sessions::record.eq(&session_clone))
                        .execute(&mut conn)
                        .map_err(DieselOrStore::Diesel)?;
                    Ok(())
                })
                .await;

            drop(permit);

            match result {
                Ok(Ok(())) => return Ok(()),
                Ok(Err(DieselOrStore::Diesel(ref e)))
                    if is_retriable_sqlite_error(e) && attempt < MAX_RETRIES =>
                {
                    let delay_ms = 10 * 2u64.pow(attempt);
                    warn!(
                        "Session write failed (attempt {}/{}): {e}. Retrying in {delay_ms}ms...",
                        attempt + 1,
                        MAX_RETRIES + 1,
                    );
                    tokio::time::sleep(std::time::Duration::from_millis(delay_ms)).await;
                    continue;
                }
                Ok(Err(e)) => return Err(e.into()),
                Err(e) => return Err(StoreError::Database(Box::new(e))),
            }
        }

        Err(StoreError::RetriesExhausted {
            op: format!("session_write (after {} attempts)", MAX_RETRIES + 1),
        })
    }

    pub async fn delete_session_for_device(&self, address: &str, device_id: i32) -> Result<()> {
        let pool = self.pool.clone();
        let address_owned = address.to_string();

        tokio::task::spawn_blocking(move || -> Result<()> {
            let mut conn = pool
                .get()
                .map_err(|e| StoreError::Connection(Box::new(e)))?;
            diesel::delete(
                sessions::table
                    .filter(sessions::address.eq(address_owned))
                    .filter(sessions::device_id.eq(device_id)),
            )
            .execute(&mut conn)
            .map_err(|e| StoreError::Database(Box::new(e)))?;
            Ok(())
        })
        .await
        .map_err(|e| StoreError::Database(Box::new(e)))??;

        Ok(())
    }

    pub async fn put_sender_key_for_device(
        &self,
        address: &str,
        record: &[u8],
        device_id: i32,
    ) -> Result<()> {
        let pool = self.pool.clone();
        let address = address.to_string();
        let record_vec = record.to_vec();
        tokio::task::spawn_blocking(move || -> Result<()> {
            let mut conn = pool
                .get()
                .map_err(|e| StoreError::Connection(Box::new(e)))?;
            diesel::insert_into(sender_keys::table)
                .values((
                    sender_keys::address.eq(address),
                    sender_keys::record.eq(&record_vec),
                    sender_keys::device_id.eq(device_id),
                ))
                .on_conflict((sender_keys::address, sender_keys::device_id))
                .do_update()
                .set(sender_keys::record.eq(&record_vec))
                .execute(&mut conn)
                .map_err(|e| StoreError::Database(Box::new(e)))?;
            Ok(())
        })
        .await
        .map_err(|e| StoreError::Database(Box::new(e)))??;
        Ok(())
    }

    pub async fn get_sender_key_for_device(
        &self,
        address: &str,
        device_id: i32,
    ) -> Result<Option<Vec<u8>>> {
        let pool = self.pool.clone();
        let address = address.to_string();
        tokio::task::spawn_blocking(move || -> Result<Option<Vec<u8>>> {
            let mut conn = pool
                .get()
                .map_err(|e| StoreError::Connection(Box::new(e)))?;
            let res: Option<Vec<u8>> = sender_keys::table
                .select(sender_keys::record)
                .filter(sender_keys::address.eq(address))
                .filter(sender_keys::device_id.eq(device_id))
                .first(&mut conn)
                .optional()
                .map_err(|e| StoreError::Database(Box::new(e)))?;
            Ok(res)
        })
        .await
        .map_err(|e| StoreError::Database(Box::new(e)))?
    }

    pub async fn delete_sender_key_for_device(&self, address: &str, device_id: i32) -> Result<()> {
        let pool = self.pool.clone();
        let address = address.to_string();
        tokio::task::spawn_blocking(move || -> Result<()> {
            let mut conn = pool
                .get()
                .map_err(|e| StoreError::Connection(Box::new(e)))?;
            diesel::delete(
                sender_keys::table
                    .filter(sender_keys::address.eq(address))
                    .filter(sender_keys::device_id.eq(device_id)),
            )
            .execute(&mut conn)
            .map_err(|e| StoreError::Database(Box::new(e)))?;
            Ok(())
        })
        .await
        .map_err(|e| StoreError::Database(Box::new(e)))??;
        Ok(())
    }

    pub async fn get_app_state_sync_key_for_device(
        &self,
        key_id: &[u8],
        device_id: i32,
    ) -> Result<Option<AppStateSyncKey>> {
        let pool = self.pool.clone();
        let key_id = key_id.to_vec();
        let res: Option<Vec<u8>> =
            tokio::task::spawn_blocking(move || -> Result<Option<Vec<u8>>> {
                let mut conn = pool
                    .get()
                    .map_err(|e| StoreError::Connection(Box::new(e)))?;
                let res: Option<Vec<u8>> = app_state_keys::table
                    .select(app_state_keys::key_data)
                    .filter(app_state_keys::key_id.eq(&key_id))
                    .filter(app_state_keys::device_id.eq(device_id))
                    .first(&mut conn)
                    .optional()
                    .map_err(|e| StoreError::Database(Box::new(e)))?;
                Ok(res)
            })
            .await
            .map_err(|e| StoreError::Database(Box::new(e)))??;

        if let Some(data) = res {
            // An undecodable blob (an old bincode row or genuine corruption) is
            // treated as absent: the app-state sync path then re-requests the key,
            // the primary re-shares it, and the next set overwrites it as protobuf.
            match crate::wire::decode_app_state_sync_key(&data) {
                Ok(key) => Ok(Some(key)),
                Err(e) => {
                    warn!(
                        "app_state_sync_key blob ({} bytes) failed to decode: {e}; \
                         treating as absent, key will be re-requested",
                        data.len()
                    );
                    Ok(None)
                }
            }
        } else {
            Ok(None)
        }
    }

    pub async fn set_app_state_sync_key_for_device(
        &self,
        key_id: &[u8],
        key: AppStateSyncKey,
        device_id: i32,
    ) -> Result<()> {
        let pool = self.pool.clone();
        let key_id = key_id.to_vec();
        let data = crate::wire::encode_app_state_sync_key(&key);
        tokio::task::spawn_blocking(move || -> Result<()> {
            let mut conn = pool
                .get()
                .map_err(|e| StoreError::Connection(Box::new(e)))?;
            diesel::insert_into(app_state_keys::table)
                .values((
                    app_state_keys::key_id.eq(&key_id),
                    app_state_keys::key_data.eq(&data),
                    app_state_keys::device_id.eq(device_id),
                ))
                .on_conflict((app_state_keys::key_id, app_state_keys::device_id))
                .do_update()
                .set(app_state_keys::key_data.eq(&data))
                .execute(&mut conn)
                .map_err(|e| StoreError::Database(Box::new(e)))?;
            Ok(())
        })
        .await
        .map_err(|e| StoreError::Database(Box::new(e)))??;
        Ok(())
    }

    pub async fn get_latest_app_state_sync_key_id_for_device(
        &self,
        device_id: i32,
    ) -> Result<Option<Vec<u8>>> {
        let pool = self.pool.clone();
        let res: Option<Vec<u8>> =
            tokio::task::spawn_blocking(move || -> Result<Option<Vec<u8>>> {
                let mut conn = pool
                    .get()
                    .map_err(|e| StoreError::Connection(Box::new(e)))?;
                // Return the latest key whose blob actually decodes. A legacy bincode
                // row (or a corrupt one) reads as absent via get_sync_key but still
                // sits in the table with a possibly lexicographically-higher key_id;
                // selecting it here would make the outbound build_patch fail later in
                // get_app_state_key with KeyNotFound. Skip undecodable rows so outbound
                // mutations use the newest USABLE key.
                let candidates: Vec<(Vec<u8>, Vec<u8>)> = app_state_keys::table
                    .select((app_state_keys::key_id, app_state_keys::key_data))
                    .filter(app_state_keys::device_id.eq(device_id))
                    .order(app_state_keys::key_id.desc())
                    .load(&mut conn)
                    .map_err(|e| StoreError::Database(Box::new(e)))?;
                let res = candidates
                    .into_iter()
                    .find(|(_, data)| crate::wire::decode_app_state_sync_key(data).is_ok())
                    .map(|(key_id, _)| key_id);
                Ok(res)
            })
            .await
            .map_err(|e| StoreError::Database(Box::new(e)))??;
        Ok(res)
    }

    pub async fn get_app_state_version_for_device(
        &self,
        name: &str,
        device_id: i32,
    ) -> Result<HashState> {
        let pool = self.pool.clone();
        let name = name.to_string();
        let res: Option<Vec<u8>> =
            tokio::task::spawn_blocking(move || -> Result<Option<Vec<u8>>> {
                let mut conn = pool
                    .get()
                    .map_err(|e| StoreError::Connection(Box::new(e)))?;
                let res: Option<Vec<u8>> = app_state_versions::table
                    .select(app_state_versions::state_data)
                    .filter(app_state_versions::name.eq(name))
                    .filter(app_state_versions::device_id.eq(device_id))
                    .first(&mut conn)
                    .optional()
                    .map_err(|e| StoreError::Database(Box::new(e)))?;
                Ok(res)
            })
            .await
            .map_err(|e| StoreError::Database(Box::new(e)))??;

        if let Some(data) = res {
            // An undecodable blob (an old bincode row or corruption) resets the
            // collection to default, which simply re-syncs it from version 0.
            match crate::wire::decode_hash_state(&data) {
                Ok(state) => Ok(state),
                Err(e) => {
                    warn!(
                        "app_state_version blob ({} bytes) failed to decode: {e}; \
                         resetting to default, collection will re-sync from 0",
                        data.len()
                    );
                    Ok(HashState::default())
                }
            }
        } else {
            Ok(HashState::default())
        }
    }

    pub async fn set_app_state_version_for_device(
        &self,
        name: &str,
        state: HashState,
        device_id: i32,
    ) -> Result<()> {
        let name = name.to_string();
        let data = crate::wire::encode_hash_state(&state);
        self.with_retry("set_app_state_version", || {
            let name = name.clone();
            let data = data.clone();
            Box::new(move |conn: &mut SqliteConnection| {
                diesel::insert_into(app_state_versions::table)
                    .values((
                        app_state_versions::name.eq(&name),
                        app_state_versions::state_data.eq(&data),
                        app_state_versions::device_id.eq(device_id),
                    ))
                    .on_conflict((app_state_versions::name, app_state_versions::device_id))
                    .do_update()
                    .set(app_state_versions::state_data.eq(&data))
                    .execute(conn)?;
                Ok(())
            })
        })
        .await
    }

    pub async fn put_app_state_mutation_macs_for_device(
        &self,
        name: &str,
        version: u64,
        mutations: &[AppStateMutationMAC],
        device_id: i32,
    ) -> Result<()> {
        if mutations.is_empty() {
            return Ok(());
        }
        let name = name.to_string();
        let mutations: Vec<AppStateMutationMAC> = mutations.to_vec();
        self.with_retry("put_app_state_mutation_macs", || {
            let name = name.clone();
            let mutations = mutations.clone();
            Box::new(move |conn: &mut SqliteConnection| {
                let records: Vec<_> = mutations
                    .iter()
                    .map(|m| {
                        (
                            app_state_mutation_macs::name.eq(&name),
                            app_state_mutation_macs::version.eq(version as i64),
                            app_state_mutation_macs::index_mac.eq(&m.index_mac),
                            app_state_mutation_macs::value_mac.eq(&m.value_mac),
                            app_state_mutation_macs::device_id.eq(device_id),
                        )
                    })
                    .collect();

                // SQLite variable limit is typically 999 or 32766.
                // Each row has 5 columns. 100 rows * 5 = 500 params, which is safe.
                const CHUNK_SIZE: usize = 100;

                for chunk in records.chunks(CHUNK_SIZE) {
                    diesel::insert_into(app_state_mutation_macs::table)
                        .values(chunk)
                        .on_conflict((
                            app_state_mutation_macs::name,
                            app_state_mutation_macs::index_mac,
                            app_state_mutation_macs::device_id,
                        ))
                        .do_update()
                        .set((
                            app_state_mutation_macs::version
                                .eq(excluded(app_state_mutation_macs::version)),
                            app_state_mutation_macs::value_mac
                                .eq(excluded(app_state_mutation_macs::value_mac)),
                        ))
                        .execute(conn)?;
                }
                Ok(())
            })
        })
        .await
    }

    pub async fn delete_app_state_mutation_macs_for_device(
        &self,
        name: &str,
        index_macs: &[Vec<u8>],
        device_id: i32,
    ) -> Result<()> {
        if index_macs.is_empty() {
            return Ok(());
        }
        let name = name.to_string();
        let index_macs: Vec<Vec<u8>> = index_macs.to_vec();
        self.with_retry("delete_app_state_mutation_macs", || {
            let name = name.clone();
            let index_macs = index_macs.clone();
            Box::new(move |conn: &mut SqliteConnection| {
                // SQLite variable limit is usually 999 or higher.
                // We use a safe chunk size to stay well within limits.
                const CHUNK_SIZE: usize = 500;

                for chunk in index_macs.chunks(CHUNK_SIZE) {
                    diesel::delete(
                        app_state_mutation_macs::table.filter(
                            app_state_mutation_macs::name
                                .eq(&name)
                                .and(app_state_mutation_macs::index_mac.eq_any(chunk))
                                .and(app_state_mutation_macs::device_id.eq(device_id)),
                        ),
                    )
                    .execute(conn)?;
                }
                Ok(())
            })
        })
        .await
    }

    pub async fn get_app_state_mutation_mac_for_device(
        &self,
        name: &str,
        index_mac: &[u8],
        device_id: i32,
    ) -> Result<Option<Vec<u8>>> {
        let pool = self.pool.clone();
        let name = name.to_string();
        let index_mac = index_mac.to_vec();
        tokio::task::spawn_blocking(move || -> Result<Option<Vec<u8>>> {
            let mut conn = pool
                .get()
                .map_err(|e| StoreError::Connection(Box::new(e)))?;
            let res: Option<Vec<u8>> = app_state_mutation_macs::table
                .select(app_state_mutation_macs::value_mac)
                .filter(app_state_mutation_macs::name.eq(&name))
                .filter(app_state_mutation_macs::index_mac.eq(&index_mac))
                .filter(app_state_mutation_macs::device_id.eq(device_id))
                .first(&mut conn)
                .optional()
                .map_err(|e| StoreError::Database(Box::new(e)))?;
            Ok(res)
        })
        .await
        .map_err(|e| StoreError::Database(Box::new(e)))?
    }

    /// Batched read of previous-MAC values for many index_macs in one query
    /// (single spawn_blocking + `index_mac IN (...)`), replacing the per-mutation
    /// N+1 in appstate sync.
    pub async fn get_app_state_mutation_macs_batch_for_device(
        &self,
        name: &str,
        index_macs: &[Vec<u8>],
        device_id: i32,
    ) -> Result<std::collections::HashMap<Vec<u8>, Vec<u8>>> {
        if index_macs.is_empty() {
            return Ok(std::collections::HashMap::new());
        }
        let pool = self.pool.clone();
        let name = name.to_string();
        let index_macs: Vec<Vec<u8>> = index_macs.to_vec();
        tokio::task::spawn_blocking(
            move || -> Result<std::collections::HashMap<Vec<u8>, Vec<u8>>> {
                let mut conn = pool
                    .get()
                    .map_err(|e| StoreError::Connection(Box::new(e)))?;
                let mut out = std::collections::HashMap::with_capacity(index_macs.len());
                const CHUNK_SIZE: usize = 500;
                for chunk in index_macs.chunks(CHUNK_SIZE) {
                    let rows: Vec<(Vec<u8>, Vec<u8>)> = app_state_mutation_macs::table
                        .select((
                            app_state_mutation_macs::index_mac,
                            app_state_mutation_macs::value_mac,
                        ))
                        .filter(app_state_mutation_macs::name.eq(&name))
                        .filter(app_state_mutation_macs::index_mac.eq_any(chunk))
                        .filter(app_state_mutation_macs::device_id.eq(device_id))
                        .load(&mut conn)
                        .map_err(|e| StoreError::Database(Box::new(e)))?;
                    out.extend(rows);
                }
                Ok(out)
            },
        )
        .await
        .map_err(|e| StoreError::Database(Box::new(e)))?
    }
}

#[cfg_attr(target_arch = "wasm32", async_trait(?Send))]
#[cfg_attr(not(target_arch = "wasm32"), async_trait)]
impl SignalStore for SqliteStore {
    async fn put_identity(&self, address: &str, key: [u8; 32]) -> Result<()> {
        self.put_identity_for_device(address, key, self.device_id)
            .await
    }

    async fn put_identities_batch(&self, identities: &[(Arc<str>, [u8; 32])]) -> Result<()> {
        if identities.is_empty() {
            return Ok(());
        }

        let device_id = self.device_id;
        // `Arc<Vec>` so each retry attempt bumps a refcount instead of re-cloning
        // the whole batch.
        let batch = Arc::new(identities.to_vec());
        self.with_retry("put_identities_batch", || {
            let batch = batch.clone();
            Box::new(move |conn: &mut SqliteConnection| {
                conn.transaction(|conn| {
                    for (address, key) in batch.iter() {
                        diesel::insert_into(identities::table)
                            .values((
                                identities::address.eq(address.as_ref()),
                                identities::key.eq(&key[..]),
                                identities::device_id.eq(device_id),
                            ))
                            .on_conflict((identities::address, identities::device_id))
                            .do_update()
                            .set(identities::key.eq(&key[..]))
                            .execute(conn)?;
                    }
                    Ok(())
                })
            })
        })
        .await
    }

    async fn load_identity(&self, address: &str) -> Result<Option<[u8; 32]>> {
        let blob = self
            .load_identity_for_device(address, self.device_id)
            .await?;
        match blob {
            None => Ok(None),
            Some(v) => Ok(Some(v.try_into().map_err(|v: Vec<u8>| {
                StoreError::Validation(format!(
                    "identity key for '{}' has invalid length {} (expected 32)",
                    address,
                    v.len()
                ))
            })?)),
        }
    }

    async fn delete_identity(&self, address: &str) -> Result<()> {
        self.delete_identity_for_device(address, self.device_id)
            .await
    }

    async fn get_session(&self, address: &str) -> Result<Option<bytes::Bytes>> {
        Ok(self
            .get_session_for_device(address, self.device_id)
            .await?
            .map(bytes::Bytes::from))
    }

    async fn has_session(&self, address: &str) -> Result<bool> {
        let pool = self.pool.clone();
        let device_id = self.device_id;
        let address_owned = address.to_string();
        self.with_semaphore(move || -> Result<bool> {
            let mut conn = pool
                .get()
                .map_err(|e| StoreError::Connection(Box::new(e)))?;
            let exists = diesel::select(diesel::dsl::exists(
                sessions::table
                    .filter(sessions::address.eq(&address_owned))
                    .filter(sessions::device_id.eq(device_id)),
            ))
            .get_result(&mut conn)
            .map_err(|e| StoreError::Database(Box::new(e)))?;
            Ok(exists)
        })
        .await
    }

    async fn has_signal_state_for_user(&self, user: &str) -> Result<bool> {
        let pool = self.pool.clone();
        let device_id = self.device_id;
        // Address is `user@server` (device 0) or `user:dev@server`; `user` is a
        // numeric PN/LID so it carries no LIKE wildcards.
        let pat_at = format!("{user}@%");
        let pat_dev = format!("{user}:%");
        self.with_semaphore(move || -> Result<bool> {
            let mut conn = pool
                .get()
                .map_err(|e| StoreError::Connection(Box::new(e)))?;
            let has_session = diesel::select(diesel::dsl::exists(
                sessions::table
                    .filter(sessions::device_id.eq(device_id))
                    .filter(
                        sessions::address
                            .like(&pat_at)
                            .or(sessions::address.like(&pat_dev)),
                    ),
            ))
            .get_result::<bool>(&mut conn)
            .map_err(|e| StoreError::Database(Box::new(e)))?;
            if has_session {
                return Ok(true);
            }
            let has_identity = diesel::select(diesel::dsl::exists(
                identities::table
                    .filter(identities::device_id.eq(device_id))
                    .filter(
                        identities::address
                            .like(&pat_at)
                            .or(identities::address.like(&pat_dev)),
                    ),
            ))
            .get_result::<bool>(&mut conn)
            .map_err(|e| StoreError::Database(Box::new(e)))?;
            Ok(has_identity)
        })
        .await
    }

    async fn put_session(&self, address: &str, session: &[u8]) -> Result<()> {
        self.put_session_for_device(address, session, self.device_id)
            .await
    }

    async fn put_sessions_batch(&self, sessions: &[(Arc<str>, Bytes)]) -> Result<()> {
        if sessions.is_empty() {
            return Ok(());
        }

        let device_id = self.device_id;
        let batch = Arc::new(sessions.to_vec());
        self.with_retry("put_sessions_batch", || {
            let batch = batch.clone();
            Box::new(move |conn: &mut SqliteConnection| {
                conn.transaction(|conn| {
                    for (address, record) in batch.iter() {
                        diesel::insert_into(sessions::table)
                            .values((
                                sessions::address.eq(address.as_ref()),
                                sessions::record.eq(record.as_ref()),
                                sessions::device_id.eq(device_id),
                            ))
                            .on_conflict((sessions::address, sessions::device_id))
                            .do_update()
                            .set(sessions::record.eq(record.as_ref()))
                            .execute(conn)?;
                    }
                    Ok(())
                })
            })
        })
        .await
    }

    async fn delete_session(&self, address: &str) -> Result<()> {
        self.delete_session_for_device(address, self.device_id)
            .await
    }

    async fn store_prekey(&self, id: u32, record: &[u8], uploaded: bool) -> Result<()> {
        let pool = self.pool.clone();
        let db_semaphore = self.db_semaphore.clone();
        let device_id = self.device_id;
        let record = record.to_vec();

        const MAX_RETRIES: u32 = 5;

        for attempt in 0..=MAX_RETRIES {
            let permit = db_semaphore
                .clone()
                .acquire_owned()
                .await
                .map_err(|e| StoreError::Database(Box::new(e)))?;

            let pool_clone = pool.clone();
            let record_clone = record.clone();

            let result =
                tokio::task::spawn_blocking(move || -> std::result::Result<(), DieselOrStore> {
                    let mut conn = pool_clone
                        .get()
                        .map_err(|e| DieselOrStore::Store(StoreError::Connection(Box::new(e))))?;
                    diesel::insert_into(prekeys::table)
                        .values((
                            prekeys::id.eq(id as i32),
                            prekeys::key.eq(&record_clone),
                            prekeys::uploaded.eq(uploaded),
                            prekeys::device_id.eq(device_id),
                        ))
                        .on_conflict((prekeys::id, prekeys::device_id))
                        .do_update()
                        .set((
                            prekeys::key.eq(&record_clone),
                            prekeys::uploaded.eq(uploaded),
                        ))
                        .execute(&mut conn)
                        .map_err(DieselOrStore::Diesel)?;
                    Ok(())
                })
                .await;

            drop(permit);

            match result {
                Ok(Ok(())) => return Ok(()),
                Ok(Err(DieselOrStore::Diesel(ref e)))
                    if is_retriable_sqlite_error(e) && attempt < MAX_RETRIES =>
                {
                    let delay_ms = 10u64 * (1u64 << attempt.min(4));
                    tokio::time::sleep(tokio::time::Duration::from_millis(delay_ms)).await;
                }
                Ok(Err(e)) => return Err(e.into()),
                Err(e) => return Err(StoreError::Database(Box::new(e))),
            }
        }

        Err(StoreError::RetriesExhausted {
            op: "store_prekey".to_string(),
        })
    }

    async fn store_prekeys_batch(&self, keys: &[(u32, Bytes)], uploaded: bool) -> Result<()> {
        if keys.is_empty() {
            return Ok(());
        }

        let pool = self.pool.clone();
        let db_semaphore = self.db_semaphore.clone();
        let device_id = self.device_id;
        let keys: Vec<(u32, Bytes)> = keys.to_vec();

        const MAX_RETRIES: u32 = 5;

        for attempt in 0..=MAX_RETRIES {
            let permit = db_semaphore
                .clone()
                .acquire_owned()
                .await
                .map_err(|e| StoreError::Database(Box::new(e)))?;

            let pool_clone = pool.clone();
            let keys_clone = keys.clone();

            let result =
                tokio::task::spawn_blocking(move || -> std::result::Result<(), DieselOrStore> {
                    let mut conn = pool_clone
                        .get()
                        .map_err(|e| DieselOrStore::Store(StoreError::Connection(Box::new(e))))?;

                    conn.transaction(|conn| {
                        for (id, record) in &keys_clone {
                            diesel::insert_into(prekeys::table)
                                .values((
                                    prekeys::id.eq(*id as i32),
                                    prekeys::key.eq(record.as_ref()),
                                    prekeys::uploaded.eq(uploaded),
                                    prekeys::device_id.eq(device_id),
                                ))
                                .on_conflict((prekeys::id, prekeys::device_id))
                                .do_update()
                                .set((
                                    prekeys::key.eq(record.as_ref()),
                                    prekeys::uploaded.eq(uploaded),
                                ))
                                .execute(conn)?;
                        }
                        Ok::<(), diesel::result::Error>(())
                    })
                    .map_err(DieselOrStore::Diesel)
                })
                .await;

            drop(permit);

            match result {
                Ok(Ok(())) => return Ok(()),
                Ok(Err(DieselOrStore::Diesel(ref e)))
                    if is_retriable_sqlite_error(e) && attempt < MAX_RETRIES =>
                {
                    let delay_ms = 10u64 * (1u64 << attempt.min(4));
                    tokio::time::sleep(tokio::time::Duration::from_millis(delay_ms)).await;
                }
                Ok(Err(e)) => return Err(e.into()),
                Err(e) => return Err(StoreError::Database(Box::new(e))),
            }
        }

        Err(StoreError::RetriesExhausted {
            op: "store_prekeys_batch".to_string(),
        })
    }

    async fn load_prekey(&self, id: u32) -> Result<Option<Bytes>> {
        let pool = self.pool.clone();
        let device_id = self.device_id;
        tokio::task::spawn_blocking(move || -> Result<Option<Bytes>> {
            let mut conn = pool
                .get()
                .map_err(|e| StoreError::Connection(Box::new(e)))?;
            let res: Option<Vec<u8>> = prekeys::table
                .select(prekeys::key)
                .filter(prekeys::id.eq(id as i32))
                .filter(prekeys::device_id.eq(device_id))
                .first(&mut conn)
                .optional()
                .map_err(|e| StoreError::Database(Box::new(e)))?;
            Ok(res.map(Bytes::from))
        })
        .await
        .map_err(|e| StoreError::Database(Box::new(e)))?
    }

    async fn load_prekeys_batch(&self, ids: &[u32]) -> Result<Vec<(u32, Bytes)>> {
        if ids.is_empty() {
            return Ok(Vec::new());
        }
        let pool = self.pool.clone();
        let device_id = self.device_id;
        let ids: Vec<i32> = ids.iter().map(|&id| id as i32).collect();
        self.with_semaphore(move || -> Result<Vec<(u32, Bytes)>> {
            let mut conn = pool
                .get()
                .map_err(|e| StoreError::Connection(Box::new(e)))?;
            // Chunked like mark_prekeys_uploaded: the upload window can carry
            // more ids than SQLite's host-parameter limit.
            let mut out = Vec::with_capacity(ids.len());
            for chunk in ids.chunks(ID_PARAM_CHUNK) {
                let rows: Vec<(i32, Vec<u8>)> = prekeys::table
                    .select((prekeys::id, prekeys::key))
                    .filter(prekeys::id.eq_any(chunk))
                    .filter(prekeys::device_id.eq(device_id))
                    .load(&mut conn)
                    .map_err(|e| StoreError::Database(Box::new(e)))?;
                out.extend(
                    rows.into_iter()
                        .map(|(id, key)| (id as u32, Bytes::from(key))),
                );
            }
            Ok(out)
        })
        .await
    }

    async fn remove_prekey(&self, id: u32) -> Result<()> {
        let pool = self.pool.clone();
        let db_semaphore = self.db_semaphore.clone();
        let device_id = self.device_id;

        const MAX_RETRIES: u32 = 5;

        for attempt in 0..=MAX_RETRIES {
            let permit = db_semaphore
                .clone()
                .acquire_owned()
                .await
                .map_err(|e| StoreError::Database(Box::new(e)))?;

            let pool_clone = pool.clone();

            let result =
                tokio::task::spawn_blocking(move || -> std::result::Result<(), DieselOrStore> {
                    let mut conn = pool_clone
                        .get()
                        .map_err(|e| DieselOrStore::Store(StoreError::Connection(Box::new(e))))?;
                    diesel::delete(
                        prekeys::table
                            .filter(prekeys::id.eq(id as i32))
                            .filter(prekeys::device_id.eq(device_id)),
                    )
                    .execute(&mut conn)
                    .map_err(DieselOrStore::Diesel)?;
                    Ok(())
                })
                .await;

            drop(permit);

            match result {
                Ok(Ok(())) => return Ok(()),
                Ok(Err(DieselOrStore::Diesel(ref e)))
                    if is_retriable_sqlite_error(e) && attempt < MAX_RETRIES =>
                {
                    let delay_ms = 10u64 * (1u64 << attempt.min(4));
                    tokio::time::sleep(tokio::time::Duration::from_millis(delay_ms)).await;
                }
                Ok(Err(e)) => return Err(e.into()),
                Err(e) => return Err(StoreError::Database(Box::new(e))),
            }
        }

        Err(StoreError::RetriesExhausted {
            op: "remove_prekey".to_string(),
        })
    }

    async fn mark_prekeys_uploaded(&self, ids: &[u32]) -> Result<()> {
        if ids.is_empty() {
            return Ok(());
        }
        let device_id = self.device_id;
        let ids: Vec<i32> = ids.iter().map(|&id| id as i32).collect();
        self.with_retry("mark_prekeys_uploaded", move || {
            let ids = ids.clone();
            Box::new(move |conn: &mut SqliteConnection| {
                // Stay under SQLite's host-parameter limit (999 by default);
                // the upload batch is configurable up to u16::MAX ids.
                for chunk in ids.chunks(ID_PARAM_CHUNK) {
                    diesel::update(
                        prekeys::table
                            .filter(prekeys::id.eq_any(chunk.to_vec()))
                            .filter(prekeys::device_id.eq(device_id)),
                    )
                    .set(prekeys::uploaded.eq(true))
                    .execute(conn)?;
                }
                Ok(())
            })
        })
        .await
    }

    async fn get_max_prekey_id(&self) -> Result<u32> {
        let pool = self.pool.clone();
        let device_id = self.device_id;
        let db_semaphore = self.db_semaphore.clone();
        let _permit = db_semaphore
            .acquire()
            .await
            .map_err(|e| StoreError::Database(Box::new(e)))?;

        tokio::task::spawn_blocking(move || -> Result<u32> {
            let mut conn = pool
                .get()
                .map_err(|e| StoreError::Connection(Box::new(e)))?;
            use diesel::dsl::max;
            let result: Option<i32> = prekeys::table
                .filter(prekeys::device_id.eq(device_id))
                .select(max(prekeys::id))
                .first(&mut conn)
                .map_err(|e| StoreError::Database(Box::new(e)))?;
            Ok(result.unwrap_or(0) as u32)
        })
        .await
        .map_err(|e| StoreError::Database(Box::new(e)))?
    }

    async fn store_signed_prekey(&self, id: u32, record: &[u8]) -> Result<()> {
        let pool = self.pool.clone();
        let db_semaphore = self.db_semaphore.clone();
        let device_id = self.device_id;
        let record = record.to_vec();

        const MAX_RETRIES: u32 = 5;

        for attempt in 0..=MAX_RETRIES {
            let permit = db_semaphore
                .clone()
                .acquire_owned()
                .await
                .map_err(|e| StoreError::Database(Box::new(e)))?;

            let pool_clone = pool.clone();
            let record_clone = record.clone();

            let result =
                tokio::task::spawn_blocking(move || -> std::result::Result<(), DieselOrStore> {
                    let mut conn = pool_clone
                        .get()
                        .map_err(|e| DieselOrStore::Store(StoreError::Connection(Box::new(e))))?;
                    diesel::insert_into(signed_prekeys::table)
                        .values((
                            signed_prekeys::id.eq(id as i32),
                            signed_prekeys::record.eq(&record_clone),
                            signed_prekeys::device_id.eq(device_id),
                        ))
                        .on_conflict((signed_prekeys::id, signed_prekeys::device_id))
                        .do_update()
                        .set(signed_prekeys::record.eq(&record_clone))
                        .execute(&mut conn)
                        .map_err(DieselOrStore::Diesel)?;
                    Ok(())
                })
                .await;

            drop(permit);

            match result {
                Ok(Ok(())) => return Ok(()),
                Ok(Err(DieselOrStore::Diesel(ref e)))
                    if is_retriable_sqlite_error(e) && attempt < MAX_RETRIES =>
                {
                    let delay_ms = 10u64 * (1u64 << attempt.min(4));
                    tokio::time::sleep(tokio::time::Duration::from_millis(delay_ms)).await;
                }
                Ok(Err(e)) => return Err(e.into()),
                Err(e) => return Err(StoreError::Database(Box::new(e))),
            }
        }

        Err(StoreError::RetriesExhausted {
            op: "store_signed_prekey".to_string(),
        })
    }

    async fn load_signed_prekey(&self, id: u32) -> Result<Option<Vec<u8>>> {
        let pool = self.pool.clone();
        let device_id = self.device_id;
        tokio::task::spawn_blocking(move || -> Result<Option<Vec<u8>>> {
            let mut conn = pool
                .get()
                .map_err(|e| StoreError::Connection(Box::new(e)))?;
            let res: Option<Vec<u8>> = signed_prekeys::table
                .select(signed_prekeys::record)
                .filter(signed_prekeys::id.eq(id as i32))
                .filter(signed_prekeys::device_id.eq(device_id))
                .first(&mut conn)
                .optional()
                .map_err(|e| StoreError::Database(Box::new(e)))?;
            Ok(res)
        })
        .await
        .map_err(|e| StoreError::Database(Box::new(e)))?
    }

    async fn load_all_signed_prekeys(&self) -> Result<Vec<(u32, Vec<u8>)>> {
        let pool = self.pool.clone();
        let device_id = self.device_id;
        tokio::task::spawn_blocking(move || -> Result<Vec<(u32, Vec<u8>)>> {
            let mut conn = pool
                .get()
                .map_err(|e| StoreError::Connection(Box::new(e)))?;
            let results: Vec<(i32, Vec<u8>)> = signed_prekeys::table
                .select((signed_prekeys::id, signed_prekeys::record))
                .filter(signed_prekeys::device_id.eq(device_id))
                .load(&mut conn)
                .map_err(|e| StoreError::Database(Box::new(e)))?;
            Ok(results
                .into_iter()
                .map(|(id, record)| (id as u32, record))
                .collect())
        })
        .await
        .map_err(|e| StoreError::Database(Box::new(e)))?
    }

    async fn remove_signed_prekey(&self, id: u32) -> Result<()> {
        let pool = self.pool.clone();
        let db_semaphore = self.db_semaphore.clone();
        let device_id = self.device_id;

        const MAX_RETRIES: u32 = 5;

        for attempt in 0..=MAX_RETRIES {
            let permit = db_semaphore
                .clone()
                .acquire_owned()
                .await
                .map_err(|e| StoreError::Database(Box::new(e)))?;

            let pool_clone = pool.clone();

            let result =
                tokio::task::spawn_blocking(move || -> std::result::Result<(), DieselOrStore> {
                    let mut conn = pool_clone
                        .get()
                        .map_err(|e| DieselOrStore::Store(StoreError::Connection(Box::new(e))))?;
                    diesel::delete(
                        signed_prekeys::table
                            .filter(signed_prekeys::id.eq(id as i32))
                            .filter(signed_prekeys::device_id.eq(device_id)),
                    )
                    .execute(&mut conn)
                    .map_err(DieselOrStore::Diesel)?;
                    Ok(())
                })
                .await;

            drop(permit);

            match result {
                Ok(Ok(())) => return Ok(()),
                Ok(Err(DieselOrStore::Diesel(ref e)))
                    if is_retriable_sqlite_error(e) && attempt < MAX_RETRIES =>
                {
                    let delay_ms = 10u64 * (1u64 << attempt.min(4));
                    tokio::time::sleep(tokio::time::Duration::from_millis(delay_ms)).await;
                }
                Ok(Err(e)) => return Err(e.into()),
                Err(e) => return Err(StoreError::Database(Box::new(e))),
            }
        }

        Err(StoreError::RetriesExhausted {
            op: "remove_signed_prekey".to_string(),
        })
    }

    async fn put_sender_key(&self, address: &str, record: &[u8]) -> Result<()> {
        self.put_sender_key_for_device(address, record, self.device_id)
            .await
    }

    async fn put_sender_keys_batch(&self, sender_keys: &[(Arc<str>, Bytes)]) -> Result<()> {
        if sender_keys.is_empty() {
            return Ok(());
        }

        let device_id = self.device_id;
        let batch = Arc::new(sender_keys.to_vec());
        self.with_retry("put_sender_keys_batch", || {
            let batch = batch.clone();
            Box::new(move |conn: &mut SqliteConnection| {
                conn.transaction(|conn| {
                    for (address, record) in batch.iter() {
                        diesel::insert_into(sender_keys::table)
                            .values((
                                sender_keys::address.eq(address.as_ref()),
                                sender_keys::record.eq(record.as_ref()),
                                sender_keys::device_id.eq(device_id),
                            ))
                            .on_conflict((sender_keys::address, sender_keys::device_id))
                            .do_update()
                            .set(sender_keys::record.eq(record.as_ref()))
                            .execute(conn)?;
                    }
                    Ok(())
                })
            })
        })
        .await
    }

    async fn get_sender_key(&self, address: &str) -> Result<Option<Vec<u8>>> {
        self.get_sender_key_for_device(address, self.device_id)
            .await
    }

    async fn delete_sender_key(&self, address: &str) -> Result<()> {
        self.delete_sender_key_for_device(address, self.device_id)
            .await
    }
}

#[cfg_attr(target_arch = "wasm32", async_trait(?Send))]
#[cfg_attr(not(target_arch = "wasm32"), async_trait)]
impl AppSyncStore for SqliteStore {
    async fn get_sync_key(&self, key_id: &[u8]) -> Result<Option<AppStateSyncKey>> {
        self.get_app_state_sync_key_for_device(key_id, self.device_id)
            .await
    }

    async fn set_sync_key(&self, key_id: &[u8], key: AppStateSyncKey) -> Result<()> {
        self.set_app_state_sync_key_for_device(key_id, key, self.device_id)
            .await
    }

    async fn get_version(&self, name: &str) -> Result<HashState> {
        self.get_app_state_version_for_device(name, self.device_id)
            .await
    }

    async fn set_version(&self, name: &str, state: HashState) -> Result<()> {
        self.set_app_state_version_for_device(name, state, self.device_id)
            .await
    }

    async fn put_mutation_macs(
        &self,
        name: &str,
        version: u64,
        mutations: &[AppStateMutationMAC],
    ) -> Result<()> {
        self.put_app_state_mutation_macs_for_device(name, version, mutations, self.device_id)
            .await
    }

    async fn get_mutation_mac(&self, name: &str, index_mac: &[u8]) -> Result<Option<Vec<u8>>> {
        self.get_app_state_mutation_mac_for_device(name, index_mac, self.device_id)
            .await
    }

    async fn get_mutation_macs(
        &self,
        name: &str,
        index_macs: &[Vec<u8>],
    ) -> Result<std::collections::HashMap<Vec<u8>, Vec<u8>>> {
        self.get_app_state_mutation_macs_batch_for_device(name, index_macs, self.device_id)
            .await
    }

    async fn delete_mutation_macs(&self, name: &str, index_macs: &[Vec<u8>]) -> Result<()> {
        self.delete_app_state_mutation_macs_for_device(name, index_macs, self.device_id)
            .await
    }

    async fn clear_mutation_macs(&self, name: &str) -> Result<()> {
        let device_id = self.device_id;
        let name = name.to_string();
        self.with_retry("clear_mutation_macs", || {
            let name = name.clone();
            Box::new(move |conn: &mut SqliteConnection| {
                diesel::delete(
                    app_state_mutation_macs::table
                        .filter(app_state_mutation_macs::name.eq(&name))
                        .filter(app_state_mutation_macs::device_id.eq(device_id)),
                )
                .execute(conn)?;
                Ok(())
            })
        })
        .await
    }

    async fn get_latest_sync_key_id(&self) -> Result<Option<Vec<u8>>> {
        self.get_latest_app_state_sync_key_id_for_device(self.device_id)
            .await
    }
}

#[cfg_attr(target_arch = "wasm32", async_trait(?Send))]
#[cfg_attr(not(target_arch = "wasm32"), async_trait)]
impl ProtocolStore for SqliteStore {
    async fn get_sender_key_devices(&self, group_jid: &str) -> Result<Vec<(String, bool)>> {
        let pool = self.pool.clone();
        let device_id = self.device_id;
        let group_jid = group_jid.to_string();
        tokio::task::spawn_blocking(move || -> Result<Vec<(String, bool)>> {
            let mut conn = pool
                .get()
                .map_err(|e| StoreError::Connection(Box::new(e)))?;
            let rows: Vec<(String, i32)> = sender_key_devices::table
                .select((sender_key_devices::device_jid, sender_key_devices::has_key))
                .filter(sender_key_devices::group_jid.eq(&group_jid))
                .filter(sender_key_devices::device_id.eq(device_id))
                .load(&mut conn)
                .map_err(|e| StoreError::Database(Box::new(e)))?;
            Ok(rows
                .into_iter()
                .map(|(jid, has_key)| (jid, has_key != 0))
                .collect())
        })
        .await
        .map_err(|e| StoreError::Database(Box::new(e)))?
    }

    async fn set_sender_key_status(&self, group_jid: &str, entries: &[(&str, bool)]) -> Result<()> {
        if entries.is_empty() {
            return Ok(());
        }
        let device_id = self.device_id;
        let group_jid = group_jid.to_string();
        let owned_entries: Arc<Vec<(String, bool)>> = Arc::new(
            entries
                .iter()
                .map(|(jid, has_key)| (jid.to_string(), *has_key))
                .collect(),
        );
        let now = wacore::time::now_secs();
        self.with_retry("set_sender_key_status", || {
            let group_jid = group_jid.clone();
            let owned_entries = Arc::clone(&owned_entries);
            Box::new(move |conn: &mut SqliteConnection| {
                let values: Vec<_> = owned_entries
                    .iter()
                    .map(|(device_jid, has_key)| {
                        (
                            sender_key_devices::group_jid.eq(&group_jid),
                            sender_key_devices::device_jid.eq(device_jid),
                            sender_key_devices::has_key.eq(i32::from(*has_key)),
                            sender_key_devices::device_id.eq(device_id),
                            sender_key_devices::updated_at.eq(now),
                        )
                    })
                    .collect();

                const CHUNK_SIZE: usize = 190;

                for chunk in values.chunks(CHUNK_SIZE) {
                    diesel::insert_into(sender_key_devices::table)
                        .values(chunk)
                        .on_conflict((
                            sender_key_devices::group_jid,
                            sender_key_devices::device_jid,
                            sender_key_devices::device_id,
                        ))
                        .do_update()
                        .set((
                            sender_key_devices::has_key
                                .eq(diesel::upsert::excluded(sender_key_devices::has_key)),
                            sender_key_devices::updated_at.eq(now),
                        ))
                        .execute(conn)?;
                }
                Ok(())
            })
        })
        .await
    }

    async fn clear_sender_key_devices(&self, group_jid: &str) -> Result<()> {
        let device_id = self.device_id;
        let group_jid = group_jid.to_string();
        self.with_retry("clear_sender_key_devices", || {
            let group_jid = group_jid.clone();
            Box::new(move |conn: &mut SqliteConnection| {
                diesel::delete(
                    sender_key_devices::table
                        .filter(sender_key_devices::group_jid.eq(&group_jid))
                        .filter(sender_key_devices::device_id.eq(device_id)),
                )
                .execute(conn)?;
                Ok(())
            })
        })
        .await
    }

    async fn clear_all_sender_key_devices(&self) -> Result<()> {
        let device_id = self.device_id;
        self.with_retry("clear_all_sender_key_devices", || {
            Box::new(move |conn: &mut SqliteConnection| {
                diesel::delete(
                    sender_key_devices::table.filter(sender_key_devices::device_id.eq(device_id)),
                )
                .execute(conn)?;
                Ok(())
            })
        })
        .await
    }

    async fn delete_sender_key_device_rows(&self, device_jids: &[&str]) -> Result<()> {
        if device_jids.is_empty() {
            return Ok(());
        }
        let device_id = self.device_id;
        let owned: Arc<Vec<String>> = Arc::new(device_jids.iter().map(|s| s.to_string()).collect());
        self.with_retry("delete_sender_key_device_rows", || {
            let owned = Arc::clone(&owned);
            Box::new(move |conn: &mut SqliteConnection| {
                const CHUNK: usize = 190;
                for chunk in owned.chunks(CHUNK) {
                    diesel::delete(
                        sender_key_devices::table
                            .filter(sender_key_devices::device_jid.eq_any(chunk))
                            .filter(sender_key_devices::device_id.eq(device_id)),
                    )
                    .execute(conn)?;
                }
                Ok(())
            })
        })
        .await
    }

    async fn get_lid_mapping(&self, lid: &str) -> Result<Option<LidPnMappingEntry>> {
        let pool = self.pool.clone();
        let device_id = self.device_id;
        let lid = lid.to_string();
        tokio::task::spawn_blocking(move || -> Result<Option<LidPnMappingEntry>> {
            let mut conn = pool
                .get()
                .map_err(|e| StoreError::Connection(Box::new(e)))?;
            let row: Option<(String, String, i64, String, i64)> = lid_pn_mapping::table
                .select((
                    lid_pn_mapping::lid,
                    lid_pn_mapping::phone_number,
                    lid_pn_mapping::created_at,
                    lid_pn_mapping::learning_source,
                    lid_pn_mapping::updated_at,
                ))
                .filter(lid_pn_mapping::lid.eq(&lid))
                .filter(lid_pn_mapping::device_id.eq(device_id))
                .first(&mut conn)
                .optional()
                .map_err(|e| StoreError::Database(Box::new(e)))?;
            Ok(row.map(
                |(lid, phone_number, created_at, learning_source, updated_at)| LidPnMappingEntry {
                    lid,
                    phone_number,
                    created_at,
                    updated_at,
                    learning_source,
                },
            ))
        })
        .await
        .map_err(|e| StoreError::Database(Box::new(e)))?
    }

    async fn get_pn_mapping(&self, phone: &str) -> Result<Option<LidPnMappingEntry>> {
        let pool = self.pool.clone();
        let device_id = self.device_id;
        let phone = phone.to_string();
        tokio::task::spawn_blocking(move || -> Result<Option<LidPnMappingEntry>> {
            let mut conn = pool
                .get()
                .map_err(|e| StoreError::Connection(Box::new(e)))?;
            let row: Option<(String, String, i64, String, i64)> = lid_pn_mapping::table
                .select((
                    lid_pn_mapping::lid,
                    lid_pn_mapping::phone_number,
                    lid_pn_mapping::created_at,
                    lid_pn_mapping::learning_source,
                    lid_pn_mapping::updated_at,
                ))
                .filter(lid_pn_mapping::phone_number.eq(&phone))
                .filter(lid_pn_mapping::device_id.eq(device_id))
                .order(lid_pn_mapping::updated_at.desc())
                .first(&mut conn)
                .optional()
                .map_err(|e| StoreError::Database(Box::new(e)))?;
            Ok(row.map(
                |(lid, phone_number, created_at, learning_source, updated_at)| LidPnMappingEntry {
                    lid,
                    phone_number,
                    created_at,
                    updated_at,
                    learning_source,
                },
            ))
        })
        .await
        .map_err(|e| StoreError::Database(Box::new(e)))?
    }

    async fn put_lid_mapping(&self, entry: &LidPnMappingEntry) -> Result<()> {
        self.put_lid_mappings(std::slice::from_ref(entry)).await
    }

    async fn put_lid_mappings(&self, entries: &[LidPnMappingEntry]) -> Result<()> {
        if entries.is_empty() {
            return Ok(());
        }
        let device_id = self.device_id;
        // Share the batch across retry attempts via Arc so no retry re-clones
        // the Vec. `with_retry` invokes `make_op` once per attempt; we only
        // bump the Arc refcount.
        let entries: std::sync::Arc<Vec<LidPnMappingEntry>> = std::sync::Arc::new(entries.to_vec());
        self.with_retry("put_lid_mappings", move || {
            let entries = std::sync::Arc::clone(&entries);
            Box::new(move |conn: &mut SqliteConnection| {
                conn.transaction::<_, DieselError, _>(|conn| {
                    for entry in entries.iter() {
                        diesel::insert_into(lid_pn_mapping::table)
                            .values((
                                lid_pn_mapping::lid.eq(&entry.lid),
                                lid_pn_mapping::phone_number.eq(&entry.phone_number),
                                lid_pn_mapping::created_at.eq(entry.created_at),
                                lid_pn_mapping::learning_source.eq(&entry.learning_source),
                                lid_pn_mapping::updated_at.eq(entry.updated_at),
                                lid_pn_mapping::device_id.eq(device_id),
                            ))
                            .on_conflict((lid_pn_mapping::lid, lid_pn_mapping::device_id))
                            .do_update()
                            .set((
                                lid_pn_mapping::phone_number.eq(&entry.phone_number),
                                lid_pn_mapping::learning_source.eq(&entry.learning_source),
                                lid_pn_mapping::updated_at.eq(entry.updated_at),
                            ))
                            .execute(conn)?;
                    }
                    Ok(())
                })
            })
        })
        .await
    }

    async fn get_all_lid_mappings(&self) -> Result<Vec<LidPnMappingEntry>> {
        let pool = self.pool.clone();
        let device_id = self.device_id;
        tokio::task::spawn_blocking(move || -> Result<Vec<LidPnMappingEntry>> {
            let mut conn = pool
                .get()
                .map_err(|e| StoreError::Connection(Box::new(e)))?;
            let rows: Vec<(String, String, i64, String, i64)> = lid_pn_mapping::table
                .select((
                    lid_pn_mapping::lid,
                    lid_pn_mapping::phone_number,
                    lid_pn_mapping::created_at,
                    lid_pn_mapping::learning_source,
                    lid_pn_mapping::updated_at,
                ))
                .filter(lid_pn_mapping::device_id.eq(device_id))
                .load(&mut conn)
                .map_err(|e| StoreError::Database(Box::new(e)))?;
            Ok(rows
                .into_iter()
                .map(
                    |(lid, phone_number, created_at, learning_source, updated_at)| {
                        LidPnMappingEntry {
                            lid,
                            phone_number,
                            created_at,
                            updated_at,
                            learning_source,
                        }
                    },
                )
                .collect())
        })
        .await
        .map_err(|e| StoreError::Database(Box::new(e)))?
    }

    async fn save_base_key(&self, address: &str, message_id: &str, base_key: &[u8]) -> Result<()> {
        let pool = self.pool.clone();
        let device_id = self.device_id;
        let address = address.to_string();
        let message_id = message_id.to_string();
        let base_key = base_key.to_vec();
        let now = wacore::time::now_secs() as i32;
        tokio::task::spawn_blocking(move || -> Result<()> {
            let mut conn = pool
                .get()
                .map_err(|e| StoreError::Connection(Box::new(e)))?;
            diesel::insert_into(base_keys::table)
                .values((
                    base_keys::address.eq(&address),
                    base_keys::message_id.eq(&message_id),
                    base_keys::base_key.eq(&base_key),
                    base_keys::device_id.eq(device_id),
                    base_keys::created_at.eq(now),
                ))
                .on_conflict((
                    base_keys::address,
                    base_keys::message_id,
                    base_keys::device_id,
                ))
                .do_update()
                .set(base_keys::base_key.eq(&base_key))
                .execute(&mut conn)
                .map_err(|e| StoreError::Database(Box::new(e)))?;
            Ok(())
        })
        .await
        .map_err(|e| StoreError::Database(Box::new(e)))??;
        Ok(())
    }

    async fn has_same_base_key(
        &self,
        address: &str,
        message_id: &str,
        current_base_key: &[u8],
    ) -> Result<bool> {
        let pool = self.pool.clone();
        let device_id = self.device_id;
        let address = address.to_string();
        let message_id = message_id.to_string();
        let current_base_key = current_base_key.to_vec();
        tokio::task::spawn_blocking(move || -> Result<bool> {
            let mut conn = pool
                .get()
                .map_err(|e| StoreError::Connection(Box::new(e)))?;
            let stored_key: Option<Vec<u8>> = base_keys::table
                .select(base_keys::base_key)
                .filter(base_keys::address.eq(&address))
                .filter(base_keys::message_id.eq(&message_id))
                .filter(base_keys::device_id.eq(device_id))
                .first(&mut conn)
                .optional()
                .map_err(|e| StoreError::Database(Box::new(e)))?;
            Ok(stored_key.as_ref() == Some(&current_base_key))
        })
        .await
        .map_err(|e| StoreError::Database(Box::new(e)))?
    }

    async fn delete_base_key(&self, address: &str, message_id: &str) -> Result<()> {
        let pool = self.pool.clone();
        let device_id = self.device_id;
        let address = address.to_string();
        let message_id = message_id.to_string();
        tokio::task::spawn_blocking(move || -> Result<()> {
            let mut conn = pool
                .get()
                .map_err(|e| StoreError::Connection(Box::new(e)))?;
            diesel::delete(
                base_keys::table
                    .filter(base_keys::address.eq(&address))
                    .filter(base_keys::message_id.eq(&message_id))
                    .filter(base_keys::device_id.eq(device_id)),
            )
            .execute(&mut conn)
            .map_err(|e| StoreError::Database(Box::new(e)))?;
            Ok(())
        })
        .await
        .map_err(|e| StoreError::Database(Box::new(e)))??;
        Ok(())
    }

    async fn update_device_list(&self, record: DeviceListRecord) -> Result<()> {
        let pool = self.pool.clone();
        let device_id = self.device_id;
        let devices_json = serde_json::to_string(&record.devices)
            .map_err(|e| StoreError::Serialization(Box::new(e)))?;
        let now = wacore::time::now_secs() as i32;
        tokio::task::spawn_blocking(move || -> Result<()> {
            let mut conn = pool
                .get()
                .map_err(|e| StoreError::Connection(Box::new(e)))?;
            let raw_id_i32 = record.raw_id.map(|r| r as i32);
            diesel::insert_into(device_registry::table)
                .values((
                    device_registry::user_id.eq(&record.user),
                    device_registry::devices_json.eq(&devices_json),
                    device_registry::timestamp.eq(record.timestamp as i32),
                    device_registry::phash.eq(&record.phash),
                    device_registry::device_id.eq(device_id),
                    device_registry::updated_at.eq(now),
                    device_registry::raw_id.eq(raw_id_i32),
                ))
                .on_conflict((device_registry::user_id, device_registry::device_id))
                .do_update()
                .set((
                    device_registry::devices_json.eq(&devices_json),
                    device_registry::timestamp.eq(record.timestamp as i32),
                    device_registry::phash.eq(&record.phash),
                    device_registry::updated_at.eq(now),
                    device_registry::raw_id.eq(raw_id_i32),
                ))
                .execute(&mut conn)
                .map_err(|e| StoreError::Database(Box::new(e)))?;
            Ok(())
        })
        .await
        .map_err(|e| StoreError::Database(Box::new(e)))??;
        Ok(())
    }

    async fn update_device_lists(&self, records: Vec<DeviceListRecord>) -> Result<()> {
        if records.is_empty() {
            return Ok(());
        }
        let device_id = self.device_id;
        let now = wacore::time::now_secs() as i32;

        // Pre-serialize devices_json once (outside the retry loop and outside
        // spawn_blocking) so retries are zero-allocation. Each row carries its
        // own json+raw_id alongside the record.
        struct PreparedRow {
            user: String,
            devices_json: String,
            timestamp: i32,
            phash: Option<String>,
            raw_id: Option<i32>,
        }

        let prepared: Vec<PreparedRow> = records
            .into_iter()
            .map(|r| {
                let devices_json = serde_json::to_string(&r.devices)
                    .map_err(|e| StoreError::Serialization(Box::new(e)))?;
                Ok(PreparedRow {
                    user: r.user,
                    devices_json,
                    timestamp: r.timestamp as i32,
                    phash: r.phash,
                    raw_id: r.raw_id.map(|v| v as i32),
                })
            })
            .collect::<Result<Vec<_>>>()?;
        let prepared = std::sync::Arc::new(prepared);

        self.with_retry("update_device_lists", move || {
            let prepared = std::sync::Arc::clone(&prepared);
            Box::new(move |conn: &mut SqliteConnection| {
                conn.transaction::<_, DieselError, _>(|conn| {
                    for row in prepared.iter() {
                        diesel::insert_into(device_registry::table)
                            .values((
                                device_registry::user_id.eq(&row.user),
                                device_registry::devices_json.eq(&row.devices_json),
                                device_registry::timestamp.eq(row.timestamp),
                                device_registry::phash.eq(&row.phash),
                                device_registry::device_id.eq(device_id),
                                device_registry::updated_at.eq(now),
                                device_registry::raw_id.eq(row.raw_id),
                            ))
                            .on_conflict((device_registry::user_id, device_registry::device_id))
                            .do_update()
                            .set((
                                device_registry::devices_json.eq(&row.devices_json),
                                device_registry::timestamp.eq(row.timestamp),
                                device_registry::phash.eq(&row.phash),
                                device_registry::updated_at.eq(now),
                                device_registry::raw_id.eq(row.raw_id),
                            ))
                            .execute(conn)?;
                    }
                    Ok(())
                })
            })
        })
        .await
    }

    async fn get_devices(&self, user: &str) -> Result<Option<DeviceListRecord>> {
        let pool = self.pool.clone();
        let device_id = self.device_id;
        let user = user.to_string();
        tokio::task::spawn_blocking(move || -> Result<Option<DeviceListRecord>> {
            let mut conn = pool
                .get()
                .map_err(|e| StoreError::Connection(Box::new(e)))?;
            let row: Option<(String, String, i32, Option<String>, Option<i32>)> =
                device_registry::table
                    .select((
                        device_registry::user_id,
                        device_registry::devices_json,
                        device_registry::timestamp,
                        device_registry::phash,
                        device_registry::raw_id,
                    ))
                    .filter(device_registry::user_id.eq(&user))
                    .filter(device_registry::device_id.eq(device_id))
                    .first(&mut conn)
                    .optional()
                    .map_err(|e| StoreError::Database(Box::new(e)))?;
            match row {
                Some((user, devices_json, timestamp, phash, raw_id)) => {
                    let devices: Vec<DeviceInfo> = serde_json::from_str(&devices_json)
                        .map_err(|e| StoreError::Serialization(Box::new(e)))?;
                    Ok(Some(DeviceListRecord {
                        user,
                        devices,
                        timestamp: timestamp as i64,
                        phash,
                        raw_id: raw_id.map(|r| r as u32),
                    }))
                }
                None => Ok(None),
            }
        })
        .await
        .map_err(|e| StoreError::Database(Box::new(e)))?
    }

    async fn delete_devices(&self, user: &str) -> Result<()> {
        let pool = self.pool.clone();
        let device_id = self.device_id;
        let user = user.to_string();
        tokio::task::spawn_blocking(move || -> Result<()> {
            let mut conn = pool
                .get()
                .map_err(|e| StoreError::Connection(Box::new(e)))?;
            diesel::delete(
                device_registry::table
                    .filter(device_registry::user_id.eq(&user))
                    .filter(device_registry::device_id.eq(device_id)),
            )
            .execute(&mut conn)
            .map_err(|e| StoreError::Database(Box::new(e)))?;
            Ok(())
        })
        .await
        .map_err(|e| StoreError::Database(Box::new(e)))??;
        Ok(())
    }

    async fn get_group_metadata(&self, group_jid: &str) -> Result<Option<Vec<u8>>> {
        let pool = self.pool.clone();
        let device_id = self.device_id;
        let group_jid = group_jid.to_string();
        tokio::task::spawn_blocking(move || -> Result<Option<Vec<u8>>> {
            let mut conn = pool
                .get()
                .map_err(|e| StoreError::Connection(Box::new(e)))?;
            let row: Option<Vec<u8>> = group_metadata::table
                .select(group_metadata::info)
                .filter(group_metadata::group_jid.eq(&group_jid))
                .filter(group_metadata::device_id.eq(device_id))
                .first(&mut conn)
                .optional()
                .map_err(|e| StoreError::Database(Box::new(e)))?;
            Ok(row)
        })
        .await
        .map_err(|e| StoreError::Database(Box::new(e)))?
    }

    async fn put_group_metadata(&self, group_jid: &str, blob: &[u8]) -> Result<()> {
        let pool = self.pool.clone();
        let device_id = self.device_id;
        let group_jid = group_jid.to_string();
        let blob = blob.to_vec();
        let now = wacore::time::now_secs();
        tokio::task::spawn_blocking(move || -> Result<()> {
            let mut conn = pool
                .get()
                .map_err(|e| StoreError::Connection(Box::new(e)))?;
            diesel::insert_into(group_metadata::table)
                .values((
                    group_metadata::group_jid.eq(&group_jid),
                    group_metadata::info.eq(&blob),
                    group_metadata::device_id.eq(device_id),
                    group_metadata::updated_at.eq(now),
                ))
                .on_conflict((group_metadata::group_jid, group_metadata::device_id))
                .do_update()
                .set((
                    group_metadata::info.eq(&blob),
                    group_metadata::updated_at.eq(now),
                ))
                .execute(&mut conn)
                .map_err(|e| StoreError::Database(Box::new(e)))?;
            Ok(())
        })
        .await
        .map_err(|e| StoreError::Database(Box::new(e)))??;
        Ok(())
    }

    async fn delete_group_metadata(&self, group_jid: &str) -> Result<()> {
        let pool = self.pool.clone();
        let device_id = self.device_id;
        let group_jid = group_jid.to_string();
        tokio::task::spawn_blocking(move || -> Result<()> {
            let mut conn = pool
                .get()
                .map_err(|e| StoreError::Connection(Box::new(e)))?;
            diesel::delete(
                group_metadata::table
                    .filter(group_metadata::group_jid.eq(&group_jid))
                    .filter(group_metadata::device_id.eq(device_id)),
            )
            .execute(&mut conn)
            .map_err(|e| StoreError::Database(Box::new(e)))?;
            Ok(())
        })
        .await
        .map_err(|e| StoreError::Database(Box::new(e)))??;
        Ok(())
    }

    async fn get_tc_token(&self, jid: &str) -> Result<Option<TcTokenEntry>> {
        let pool = self.pool.clone();
        let device_id = self.device_id;
        let jid = jid.to_string();
        tokio::task::spawn_blocking(move || -> Result<Option<TcTokenEntry>> {
            let mut conn = pool
                .get()
                .map_err(|e| StoreError::Connection(Box::new(e)))?;
            let row: Option<(Vec<u8>, i64, Option<i64>)> = tc_tokens::table
                .select((
                    tc_tokens::token,
                    tc_tokens::token_timestamp,
                    tc_tokens::sender_timestamp,
                ))
                .filter(tc_tokens::jid.eq(&jid))
                .filter(tc_tokens::device_id.eq(device_id))
                .first(&mut conn)
                .optional()
                .map_err(|e| StoreError::Database(Box::new(e)))?;
            Ok(
                row.map(|(token, token_timestamp, sender_timestamp)| TcTokenEntry {
                    token,
                    token_timestamp,
                    sender_timestamp,
                }),
            )
        })
        .await
        .map_err(|e| StoreError::Database(Box::new(e)))?
    }

    async fn put_tc_token(&self, jid: &str, entry: &TcTokenEntry) -> Result<()> {
        let pool = self.pool.clone();
        let device_id = self.device_id;
        let jid = jid.to_string();
        let entry = entry.clone();
        let now = wacore::time::now_secs();
        tokio::task::spawn_blocking(move || -> Result<()> {
            let mut conn = pool
                .get()
                .map_err(|e| StoreError::Connection(Box::new(e)))?;
            diesel::insert_into(tc_tokens::table)
                .values((
                    tc_tokens::jid.eq(&jid),
                    tc_tokens::token.eq(&entry.token),
                    tc_tokens::token_timestamp.eq(entry.token_timestamp),
                    tc_tokens::sender_timestamp.eq(entry.sender_timestamp),
                    tc_tokens::device_id.eq(device_id),
                    tc_tokens::updated_at.eq(now),
                ))
                .on_conflict((tc_tokens::jid, tc_tokens::device_id))
                .do_update()
                .set((
                    tc_tokens::token.eq(&entry.token),
                    tc_tokens::token_timestamp.eq(entry.token_timestamp),
                    tc_tokens::sender_timestamp.eq(entry.sender_timestamp),
                    tc_tokens::updated_at.eq(now),
                ))
                .execute(&mut conn)
                .map_err(|e| StoreError::Database(Box::new(e)))?;
            Ok(())
        })
        .await
        .map_err(|e| StoreError::Database(Box::new(e)))??;
        Ok(())
    }

    async fn delete_tc_token(&self, jid: &str) -> Result<()> {
        let pool = self.pool.clone();
        let device_id = self.device_id;
        let jid = jid.to_string();
        tokio::task::spawn_blocking(move || -> Result<()> {
            let mut conn = pool
                .get()
                .map_err(|e| StoreError::Connection(Box::new(e)))?;
            diesel::delete(
                tc_tokens::table
                    .filter(tc_tokens::jid.eq(&jid))
                    .filter(tc_tokens::device_id.eq(device_id)),
            )
            .execute(&mut conn)
            .map_err(|e| StoreError::Database(Box::new(e)))?;
            Ok(())
        })
        .await
        .map_err(|e| StoreError::Database(Box::new(e)))??;
        Ok(())
    }

    async fn get_all_tc_token_jids(&self) -> Result<Vec<String>> {
        let pool = self.pool.clone();
        let device_id = self.device_id;
        tokio::task::spawn_blocking(move || -> Result<Vec<String>> {
            let mut conn = pool
                .get()
                .map_err(|e| StoreError::Connection(Box::new(e)))?;
            let jids: Vec<String> = tc_tokens::table
                .select(tc_tokens::jid)
                .filter(tc_tokens::device_id.eq(device_id))
                .load(&mut conn)
                .map_err(|e| StoreError::Database(Box::new(e)))?;
            Ok(jids)
        })
        .await
        .map_err(|e| StoreError::Database(Box::new(e)))?
    }

    async fn delete_expired_tc_tokens(&self, cutoff_timestamp: i64) -> Result<u32> {
        let pool = self.pool.clone();
        let device_id = self.device_id;
        tokio::task::spawn_blocking(move || -> Result<u32> {
            let mut conn = pool
                .get()
                .map_err(|e| StoreError::Connection(Box::new(e)))?;
            let deleted = diesel::delete(
                tc_tokens::table
                    .filter(tc_tokens::token_timestamp.lt(cutoff_timestamp))
                    .filter(tc_tokens::device_id.eq(device_id)),
            )
            .execute(&mut conn)
            .map_err(|e| StoreError::Database(Box::new(e)))?;
            Ok(deleted as u32)
        })
        .await
        .map_err(|e| StoreError::Database(Box::new(e)))?
    }

    async fn store_sent_message(
        &self,
        chat_jid: &str,
        message_id: &str,
        payload: &[u8],
    ) -> Result<()> {
        let chat_jid = chat_jid.to_string();
        let message_id = message_id.to_string();
        // Arc avoids cloning the full payload bytes on each retry iteration
        let payload: Arc<Vec<u8>> = Arc::new(payload.to_vec());
        let device_id = self.device_id;
        self.with_retry("store_sent_message", || {
            let chat_jid = chat_jid.clone();
            let message_id = message_id.clone();
            let payload = Arc::clone(&payload);
            Box::new(move |conn: &mut SqliteConnection| {
                diesel::replace_into(sent_messages::table)
                    .values((
                        sent_messages::chat_jid.eq(&chat_jid),
                        sent_messages::message_id.eq(&message_id),
                        sent_messages::payload.eq(payload.as_slice()),
                        sent_messages::device_id.eq(device_id),
                    ))
                    .execute(conn)?;
                Ok(())
            })
        })
        .await
    }

    async fn take_sent_message(&self, chat_jid: &str, message_id: &str) -> Result<Option<Vec<u8>>> {
        let chat_jid = chat_jid.to_string();
        let message_id = message_id.to_string();
        let device_id = self.device_id;
        // Atomic SELECT+DELETE with retry for SQLITE_BUSY resilience.
        self.with_retry("take_sent_message", || {
            let chat_jid = chat_jid.clone();
            let message_id = message_id.clone();
            Box::new(move |conn: &mut SqliteConnection| {
                conn.immediate_transaction(|conn| {
                    let row: Option<Vec<u8>> = sent_messages::table
                        .select(sent_messages::payload)
                        .filter(sent_messages::chat_jid.eq(&chat_jid))
                        .filter(sent_messages::message_id.eq(&message_id))
                        .filter(sent_messages::device_id.eq(device_id))
                        .first(conn)
                        .optional()?;
                    if row.is_some() {
                        diesel::delete(
                            sent_messages::table
                                .filter(sent_messages::chat_jid.eq(&chat_jid))
                                .filter(sent_messages::message_id.eq(&message_id))
                                .filter(sent_messages::device_id.eq(device_id)),
                        )
                        .execute(conn)?;
                    }
                    Ok(row)
                })
            })
        })
        .await
    }

    async fn delete_expired_sent_messages(&self, cutoff_timestamp: i64) -> Result<u32> {
        let pool = self.pool.clone();
        let device_id = self.device_id;
        tokio::task::spawn_blocking(move || -> Result<u32> {
            let mut conn = pool
                .get()
                .map_err(|e| StoreError::Connection(Box::new(e)))?;
            let deleted = diesel::delete(
                sent_messages::table
                    .filter(sent_messages::created_at.lt(cutoff_timestamp))
                    .filter(sent_messages::device_id.eq(device_id)),
            )
            .execute(&mut conn)
            .map_err(|e| StoreError::Database(Box::new(e)))?;
            Ok(deleted as u32)
        })
        .await
        .map_err(|e| StoreError::Database(Box::new(e)))?
    }

    async fn store_pending_inbound(
        &self,
        chat: &str,
        sender: &str,
        id: &str,
        message: &[u8],
    ) -> Result<()> {
        let chat = chat.to_string();
        let sender = sender.to_string();
        let id = id.to_string();
        // Arc avoids cloning the payload bytes on each retry iteration.
        let message: Arc<Vec<u8>> = Arc::new(message.to_vec());
        let device_id = self.device_id;
        self.with_retry("store_pending_inbound", || {
            let chat = chat.clone();
            let sender = sender.clone();
            let id = id.clone();
            let message = Arc::clone(&message);
            Box::new(move |conn: &mut SqliteConnection| {
                diesel::replace_into(pending_inbound_messages::table)
                    .values((
                        pending_inbound_messages::chat.eq(&chat),
                        pending_inbound_messages::sender.eq(&sender),
                        pending_inbound_messages::id.eq(&id),
                        pending_inbound_messages::message.eq(message.as_slice()),
                        pending_inbound_messages::device_id.eq(device_id),
                    ))
                    .execute(conn)?;
                Ok(())
            })
        })
        .await
    }

    async fn get_pending_inbound(
        &self,
        chat: &str,
        sender: &str,
        id: &str,
    ) -> Result<Option<Vec<u8>>> {
        let chat = chat.to_string();
        let sender = sender.to_string();
        let id = id.to_string();
        let device_id = self.device_id;
        // Retry on SQLITE_BUSY: a transient lock here must not surface as a read
        // failure, which fails closed and forces an unnecessary redelivery.
        self.with_retry("get_pending_inbound", || {
            let chat = chat.clone();
            let sender = sender.clone();
            let id = id.clone();
            Box::new(move |conn: &mut SqliteConnection| {
                let row: Option<Vec<u8>> = pending_inbound_messages::table
                    .select(pending_inbound_messages::message)
                    .filter(pending_inbound_messages::chat.eq(&chat))
                    .filter(pending_inbound_messages::sender.eq(&sender))
                    .filter(pending_inbound_messages::id.eq(&id))
                    .filter(pending_inbound_messages::device_id.eq(device_id))
                    .first(conn)
                    .optional()?;
                Ok(row)
            })
        })
        .await
    }

    async fn delete_pending_inbound(&self, chat: &str, sender: &str, id: &str) -> Result<()> {
        let chat = chat.to_string();
        let sender = sender.to_string();
        let id = id.to_string();
        let device_id = self.device_id;
        self.with_retry("delete_pending_inbound", || {
            let chat = chat.clone();
            let sender = sender.clone();
            let id = id.clone();
            Box::new(move |conn: &mut SqliteConnection| {
                diesel::delete(
                    pending_inbound_messages::table
                        .filter(pending_inbound_messages::chat.eq(&chat))
                        .filter(pending_inbound_messages::sender.eq(&sender))
                        .filter(pending_inbound_messages::id.eq(&id))
                        .filter(pending_inbound_messages::device_id.eq(device_id)),
                )
                .execute(conn)?;
                Ok(())
            })
        })
        .await
    }

    async fn delete_expired_pending_inbound(&self, cutoff_timestamp: i64) -> Result<u32> {
        let pool = self.pool.clone();
        let device_id = self.device_id;
        tokio::task::spawn_blocking(move || -> Result<u32> {
            let mut conn = pool
                .get()
                .map_err(|e| StoreError::Connection(Box::new(e)))?;
            let deleted = diesel::delete(
                pending_inbound_messages::table
                    .filter(pending_inbound_messages::inserted_at.lt(cutoff_timestamp))
                    .filter(pending_inbound_messages::device_id.eq(device_id)),
            )
            .execute(&mut conn)
            .map_err(|e| StoreError::Database(Box::new(e)))?;
            Ok(deleted as u32)
        })
        .await
        .map_err(|e| StoreError::Database(Box::new(e)))?
    }
}

#[cfg_attr(target_arch = "wasm32", async_trait(?Send))]
#[cfg_attr(not(target_arch = "wasm32"), async_trait)]
impl MsgSecretStore for SqliteStore {
    async fn put_msg_secrets(&self, entries: Vec<MsgSecretEntry>) -> Result<usize> {
        if entries.is_empty() {
            return Ok(0);
        }

        let device_id = self.device_id;
        let entries: Arc<[MsgSecretEntry]> = Arc::from(entries);
        let now = wacore::time::now_secs();
        self.with_retry("put_msg_secrets", || {
            let entries = Arc::clone(&entries);
            Box::new(move |conn: &mut SqliteConnection| {
                let records: Vec<_> = entries
                    .iter()
                    .map(|entry| {
                        (
                            msg_secrets::chat.eq(entry.chat.as_str()),
                            msg_secrets::sender.eq(entry.sender.as_str()),
                            msg_secrets::msg_id.eq(entry.msg_id.as_str()),
                            msg_secrets::secret.eq(entry.secret.as_slice()),
                            msg_secrets::device_id.eq(device_id),
                            msg_secrets::created_at.eq(now),
                            msg_secrets::expires_at.eq(entry.expires_at),
                            msg_secrets::message_ts.eq(entry.message_ts),
                        )
                    })
                    .collect();

                const CHUNK_SIZE: usize = 100;

                conn.immediate_transaction(|conn| {
                    let mut stored = 0usize;
                    for chunk in records.chunks(CHUNK_SIZE) {
                        stored += diesel::insert_into(msg_secrets::table)
                            .values(chunk)
                            .on_conflict((
                                msg_secrets::chat,
                                msg_secrets::sender,
                                msg_secrets::msg_id,
                                msg_secrets::device_id,
                            ))
                            .do_update()
                            .set((
                                msg_secrets::secret.eq(excluded(msg_secrets::secret)),
                                msg_secrets::created_at.eq(now),
                                // Keep the later deadline; 0 (never) wins. Mirrors
                                // merge_msg_secret_expiry so a redelivery or edit
                                // re-persist never shortens an existing window.
                                msg_secrets::expires_at.eq(diesel::dsl::sql::<
                                    diesel::sql_types::BigInt,
                                >(
                                    "CASE WHEN msg_secrets.expires_at = 0 \
                                     OR excluded.expires_at = 0 THEN 0 \
                                     ELSE MAX(msg_secrets.expires_at, excluded.expires_at) END",
                                )),
                                // Parent event time is immutable; keep the known
                                // (non-zero / later) value across redeliveries.
                                msg_secrets::message_ts.eq(diesel::dsl::sql::<
                                    diesel::sql_types::BigInt,
                                >(
                                    "MAX(msg_secrets.message_ts, excluded.message_ts)",
                                )),
                            ))
                            .execute(conn)?;
                    }
                    Ok(stored)
                })
            })
        })
        .await
    }

    async fn get_msg_secret(
        &self,
        chat: &str,
        sender: &str,
        msg_id: &str,
    ) -> Result<Option<Vec<u8>>> {
        // Serialized through the db semaphore for the same reason as
        // get_msg_secret_with_ts: a read racing a write transaction must wait,
        // not error out as a phantom miss.
        let pool = self.pool.clone();
        let device_id = self.device_id;
        let chat = chat.to_string();
        let sender = sender.to_string();
        let msg_id = msg_id.to_string();
        self.with_semaphore(move || -> Result<Option<Vec<u8>>> {
            let mut conn = pool
                .get()
                .map_err(|e| StoreError::Connection(Box::new(e)))?;
            let row: Option<Vec<u8>> = msg_secrets::table
                .select(msg_secrets::secret)
                .filter(msg_secrets::chat.eq(&chat))
                .filter(msg_secrets::sender.eq(&sender))
                .filter(msg_secrets::msg_id.eq(&msg_id))
                .filter(msg_secrets::device_id.eq(device_id))
                .first(&mut conn)
                .optional()
                .map_err(|e| StoreError::Database(Box::new(e)))?;
            Ok(row)
        })
        .await
    }

    async fn get_msg_secret_with_ts(
        &self,
        chat: &str,
        sender: &str,
        msg_id: &str,
    ) -> Result<Option<(Vec<u8>, i64)>> {
        // Serialized through the db semaphore: a raw read racing a write
        // transaction hits the shared-cache table lock on in-memory stores
        // (SQLITE_LOCKED is not covered by busy_timeout) and callers treat the
        // error as a missing secret.
        let pool = self.pool.clone();
        let device_id = self.device_id;
        let chat = chat.to_string();
        let sender = sender.to_string();
        let msg_id = msg_id.to_string();
        self.with_semaphore(move || -> Result<Option<(Vec<u8>, i64)>> {
            let mut conn = pool
                .get()
                .map_err(|e| StoreError::Connection(Box::new(e)))?;
            let row: Option<(Vec<u8>, i64)> = msg_secrets::table
                .select((msg_secrets::secret, msg_secrets::message_ts))
                .filter(msg_secrets::chat.eq(&chat))
                .filter(msg_secrets::sender.eq(&sender))
                .filter(msg_secrets::msg_id.eq(&msg_id))
                .filter(msg_secrets::device_id.eq(device_id))
                .first(&mut conn)
                .optional()
                .map_err(|e| StoreError::Database(Box::new(e)))?;
            Ok(row)
        })
        .await
    }

    async fn delete_expired_msg_secrets(&self, cutoff_timestamp: i64) -> Result<u32> {
        let device_id = self.device_id;
        self.with_retry("delete_expired_msg_secrets", || {
            Box::new(move |conn: &mut SqliteConnection| {
                // Rows with expires_at = 0 never expire; only delete passed deadlines.
                let deleted = diesel::delete(
                    msg_secrets::table
                        .filter(msg_secrets::expires_at.ne(0))
                        .filter(msg_secrets::expires_at.le(cutoff_timestamp))
                        .filter(msg_secrets::device_id.eq(device_id)),
                )
                .execute(conn)?;
                Ok(deleted as u32)
            })
        })
        .await
    }
}

#[cfg_attr(target_arch = "wasm32", async_trait(?Send))]
#[cfg_attr(not(target_arch = "wasm32"), async_trait)]
impl DeviceStore for SqliteStore {
    async fn save(&self, device: &CoreDevice) -> Result<()> {
        SqliteStore::save_device_data_for_device(self, self.device_id, device).await
    }

    async fn load(&self) -> Result<Option<CoreDevice>> {
        SqliteStore::load_device_data_for_device(self, self.device_id).await
    }

    async fn exists(&self) -> Result<bool> {
        SqliteStore::device_exists(self, self.device_id).await
    }

    async fn create(&self) -> Result<i32> {
        SqliteStore::create_new_device(self).await
    }

    async fn snapshot_db(&self, name: &str, extra_content: Option<&[u8]>) -> Result<()> {
        fn sanitize_snapshot_name(name: &str) -> Result<String> {
            const MAX_LENGTH: usize = 100;

            let sanitized: String = name
                .chars()
                .map(|c| {
                    if c.is_ascii_alphanumeric() || c == '_' || c == '-' || c == '.' {
                        c
                    } else {
                        '_'
                    }
                })
                .collect();

            let sanitized = sanitized
                .split('.')
                .filter(|part| !part.is_empty() && *part != "..")
                .collect::<Vec<_>>()
                .join(".");

            let sanitized = sanitized.trim_matches(['/', '\\', '.']);

            if sanitized.is_empty() {
                return Err(StoreError::InvalidConfig(
                    "Snapshot name cannot be empty after sanitization".to_string(),
                ));
            }

            if sanitized.len() > MAX_LENGTH {
                return Err(StoreError::InvalidConfig(format!(
                    "Snapshot name exceeds maximum length of {} characters",
                    MAX_LENGTH
                )));
            }

            Ok(sanitized.to_string())
        }

        let sanitized_name = sanitize_snapshot_name(name)?;

        let pool = self.pool.clone();
        let db_path = self.database_path.clone();
        let extra_data = extra_content.map(|b| b.to_vec());

        tokio::task::spawn_blocking(move || -> Result<()> {
            let mut conn = pool
                .get()
                .map_err(|e| StoreError::Connection(Box::new(e)))?;

            let timestamp = wacore::time::now_secs();

            // Construct target path: db_path.snapshot-TIMESTAMP-SANITIZED_NAME
            let target_path = format!("{}.snapshot-{}-{}", db_path, timestamp, sanitized_name);

            // Use VACUUM INTO to create a consistent backup
            // Note: We escape single quotes in the path just in case
            let query = format!("VACUUM INTO '{}'", target_path.replace("'", "''"));

            diesel::sql_query(query)
                .execute(&mut conn)
                .map_err(|e| StoreError::Database(Box::new(e)))?;

            // Save extra content if provided
            if let Some(data) = extra_data {
                let extra_path = format!("{}.json", target_path);
                std::fs::write(&extra_path, data)?;
            }

            Ok(())
        })
        .await
        .map_err(|e| StoreError::Database(Box::new(e)))??;

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    async fn create_test_store() -> SqliteStore {
        use portable_atomic::AtomicU64;
        use std::sync::atomic::Ordering;
        static COUNTER: AtomicU64 = AtomicU64::new(0);
        let id = COUNTER.fetch_add(1, Ordering::Relaxed);
        let db_name = format!(
            "file:memdb_test_{}_{}?mode=memory&cache=shared",
            std::process::id(),
            id
        );
        SqliteStore::new(&db_name)
            .await
            .expect("Failed to create test store")
    }

    #[tokio::test]
    async fn batch_mutation_macs_matches_per_item() {
        let store = create_test_store().await;
        let name = "regular";
        let device_id = 1;

        let macs: Vec<AppStateMutationMAC> = (0..25u8)
            .map(|i| {
                let mut index_mac = vec![0u8; 32];
                index_mac[0] = i;
                AppStateMutationMAC {
                    index_mac,
                    value_mac: vec![i; 32],
                }
            })
            .collect();
        store
            .put_app_state_mutation_macs_for_device(name, 1, &macs, device_id)
            .await
            .unwrap();

        let mut index_macs: Vec<Vec<u8>> = macs.iter().map(|m| m.index_mac.clone()).collect();
        // an index that was never stored must be absent from the batch result
        index_macs.push(vec![0xFF; 32]);

        let batch = store
            .get_app_state_mutation_macs_batch_for_device(name, &index_macs, device_id)
            .await
            .unwrap();

        assert_eq!(batch.len(), macs.len());
        assert!(!batch.contains_key(&vec![0xFF; 32]));
        for m in &macs {
            // parity with the per-item path it replaces
            let per_item = store
                .get_app_state_mutation_mac_for_device(name, &m.index_mac, device_id)
                .await
                .unwrap();
            assert_eq!(per_item.as_ref(), batch.get(&m.index_mac));
            assert_eq!(batch.get(&m.index_mac), Some(&m.value_mac));
        }

        // empty input short-circuits to an empty map
        let empty = store
            .get_app_state_mutation_macs_batch_for_device(name, &[], device_id)
            .await
            .unwrap();
        assert!(empty.is_empty());
    }

    #[tokio::test]
    async fn clear_mutation_macs_wipes_only_named_collection() {
        let store = create_test_store().await;
        let mac = |i: u8| AppStateMutationMAC {
            index_mac: vec![i; 32],
            value_mac: vec![i; 32],
        };
        store
            .put_mutation_macs("regular", 1, &[mac(1)])
            .await
            .unwrap();
        store
            .put_mutation_macs("critical", 1, &[mac(2)])
            .await
            .unwrap();

        store.clear_mutation_macs("regular").await.unwrap();

        assert!(
            store
                .get_mutation_mac("regular", &[1; 32])
                .await
                .unwrap()
                .is_none()
        );
        assert!(
            store
                .get_mutation_mac("critical", &[2; 32])
                .await
                .unwrap()
                .is_some()
        );
    }

    #[tokio::test]
    async fn put_signal_batches_persist_and_upsert() {
        use std::sync::Arc;
        let store = create_test_store().await;

        let sessions: Vec<(Arc<str>, Bytes)> = (0..5u8)
            .map(|i| {
                (
                    Arc::from(format!("user{i}@s.whatsapp.net").as_str()),
                    Bytes::from(vec![i; 8]),
                )
            })
            .collect();
        store.put_sessions_batch(&sessions).await.unwrap();
        for (addr, bytes) in &sessions {
            assert_eq!(
                store.get_session(addr).await.unwrap().as_deref(),
                Some(bytes.as_ref())
            );
        }

        let identities: Vec<(Arc<str>, [u8; 32])> = (0..5u8)
            .map(|i| {
                (
                    Arc::from(format!("user{i}@s.whatsapp.net").as_str()),
                    [i; 32],
                )
            })
            .collect();
        store.put_identities_batch(&identities).await.unwrap();
        for (addr, key) in &identities {
            assert_eq!(store.load_identity(addr).await.unwrap(), Some(*key));
        }

        let sender_keys: Vec<(Arc<str>, Bytes)> = (0..5u8)
            .map(|i| {
                (
                    Arc::from(format!("g@g.us::user{i}").as_str()),
                    Bytes::from(vec![i; 16]),
                )
            })
            .collect();
        store.put_sender_keys_batch(&sender_keys).await.unwrap();
        for (addr, bytes) in &sender_keys {
            assert_eq!(
                store.get_sender_key(addr).await.unwrap().as_deref(),
                Some(bytes.as_ref())
            );
        }

        // Re-batching the same addresses upserts (on_conflict do_update).
        let updated: Vec<(Arc<str>, Bytes)> = sessions
            .iter()
            .map(|(addr, _)| (addr.clone(), Bytes::from(vec![0xAA; 8])))
            .collect();
        store.put_sessions_batch(&updated).await.unwrap();
        for (addr, _) in &sessions {
            assert_eq!(
                store.get_session(addr).await.unwrap().as_deref(),
                Some([0xAA; 8].as_slice())
            );
        }

        // Duplicate address within one batch: last value wins via on_conflict
        // do_update inside the single transaction.
        let dup: Arc<str> = Arc::from("dup@s.whatsapp.net");
        store
            .put_sessions_batch(&[
                (dup.clone(), Bytes::from(vec![1u8; 4])),
                (dup.clone(), Bytes::from(vec![2u8; 4])),
            ])
            .await
            .unwrap();
        assert_eq!(
            store.get_session(&dup).await.unwrap().as_deref(),
            Some([2u8; 4].as_slice())
        );

        // Empty batches short-circuit without error.
        store.put_sessions_batch(&[]).await.unwrap();
        store.put_identities_batch(&[]).await.unwrap();
        store.put_sender_keys_batch(&[]).await.unwrap();
    }

    #[test]
    fn test_parse_database_path_regular_path() {
        let path = "/var/lib/whatsapp/database.db";
        let result = parse_database_path(path).unwrap();
        assert_eq!(result, "/var/lib/whatsapp/database.db");
    }

    #[test]
    fn test_parse_database_path_with_sqlite_prefix() {
        let path = "sqlite:///var/lib/whatsapp/database.db";
        let result = parse_database_path(path).unwrap();
        assert_eq!(result, "/var/lib/whatsapp/database.db");
    }

    #[test]
    fn test_parse_database_path_with_query_params() {
        let path = "file:database.db?mode=memory&cache=shared";
        let result = parse_database_path(path).unwrap();
        assert_eq!(result, "file:database.db");
    }

    #[test]
    fn test_parse_database_path_with_fragment() {
        let path = "file:database.db#fragment";
        let result = parse_database_path(path).unwrap();
        assert_eq!(result, "file:database.db");
    }

    #[test]
    fn test_parse_database_path_with_both_query_and_fragment() {
        let path = "sqlite:///var/lib/database.db?mode=ro#backup";
        let result = parse_database_path(path).unwrap();
        assert_eq!(result, "/var/lib/database.db");
    }

    #[test]
    fn test_parse_database_path_in_memory_rejected() {
        let result = parse_database_path(":memory:");
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("not supported"));
    }

    #[test]
    fn test_parse_database_path_in_memory_with_query_rejected() {
        let result = parse_database_path(":memory:?cache=shared");
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("not supported"));
    }

    #[tokio::test]
    async fn test_device_registry_save_and_get() {
        let store = create_test_store().await;

        let record = DeviceListRecord {
            user: "1234567890".to_string(),
            devices: vec![
                DeviceInfo {
                    device_id: 0,
                    key_index: None,
                },
                DeviceInfo {
                    device_id: 1,
                    key_index: Some(42),
                },
            ],
            timestamp: 1234567890,
            phash: Some("2:abcdef".to_string()),
            raw_id: None,
        };

        store.update_device_list(record).await.expect("save failed");
        let loaded = store
            .get_devices("1234567890")
            .await
            .expect("get failed")
            .expect("record should exist");

        assert_eq!(loaded.user, "1234567890");
        assert_eq!(loaded.devices.len(), 2);
        assert_eq!(loaded.devices[0].device_id, 0);
        assert_eq!(loaded.devices[1].device_id, 1);
        assert_eq!(loaded.devices[1].key_index, Some(42));
        assert_eq!(loaded.phash, Some("2:abcdef".to_string()));
    }

    #[tokio::test]
    async fn test_device_registry_update_existing() {
        let store = create_test_store().await;

        let record1 = DeviceListRecord {
            user: "1234567890".to_string(),
            devices: vec![DeviceInfo {
                device_id: 0,
                key_index: None,
            }],
            timestamp: 1000,
            phash: Some("2:old".to_string()),
            raw_id: None,
        };
        store
            .update_device_list(record1)
            .await
            .expect("save1 failed");

        let record2 = DeviceListRecord {
            user: "1234567890".to_string(),
            devices: vec![
                DeviceInfo {
                    device_id: 0,
                    key_index: None,
                },
                DeviceInfo {
                    device_id: 2,
                    key_index: None,
                },
            ],
            timestamp: 2000,
            phash: Some("2:new".to_string()),
            raw_id: None,
        };
        store
            .update_device_list(record2)
            .await
            .expect("save2 failed");

        let loaded = store
            .get_devices("1234567890")
            .await
            .expect("get failed")
            .expect("record should exist");

        assert_eq!(loaded.devices.len(), 2);
        assert_eq!(loaded.phash, Some("2:new".to_string()));
    }

    #[tokio::test]
    async fn test_device_registry_get_nonexistent() {
        let store = create_test_store().await;
        let result = store.get_devices("nonexistent").await.expect("get failed");
        assert!(result.is_none());
    }

    #[tokio::test]
    async fn test_sender_key_devices_set_and_get() {
        let store = create_test_store().await;

        let group = "group123@g.us";

        // Set two devices: one has key, one needs SKDM
        store
            .set_sender_key_status(group, &[("user1:5@lid", true), ("user2:3@lid", false)])
            .await
            .expect("set failed");

        let devices = store
            .get_sender_key_devices(group)
            .await
            .expect("get failed");
        assert_eq!(devices.len(), 2);
        assert!(devices.contains(&("user1:5@lid".to_string(), true)));
        assert!(devices.contains(&("user2:3@lid".to_string(), false)));
    }

    #[tokio::test]
    async fn test_sender_key_devices_upsert_overwrites() {
        let store = create_test_store().await;

        let group = "group123@g.us";

        // Initially mark as needing SKDM
        store
            .set_sender_key_status(group, &[("user1:5@lid", false)])
            .await
            .expect("set failed");

        // Then mark as having key (simulates successful SKDM delivery)
        store
            .set_sender_key_status(group, &[("user1:5@lid", true)])
            .await
            .expect("set failed");

        let devices = store
            .get_sender_key_devices(group)
            .await
            .expect("get failed");
        assert_eq!(devices.len(), 1);
        assert_eq!(devices[0], ("user1:5@lid".to_string(), true));
    }

    #[tokio::test]
    async fn test_sender_key_devices_clear() {
        let store = create_test_store().await;

        let group = "group123@g.us";

        store
            .set_sender_key_status(group, &[("user1:5@lid", true), ("user2:3@lid", true)])
            .await
            .expect("set failed");

        store
            .clear_sender_key_devices(group)
            .await
            .expect("clear failed");

        let devices = store
            .get_sender_key_devices(group)
            .await
            .expect("get failed");
        assert!(devices.is_empty());
    }

    #[tokio::test]
    async fn test_tc_token_put_and_get() {
        let store = create_test_store().await;

        let entry = TcTokenEntry {
            token: vec![1, 2, 3, 4, 5],
            token_timestamp: 1707000000,
            sender_timestamp: Some(1707000100),
        };

        store
            .put_tc_token("user@lid", &entry)
            .await
            .expect("put failed");

        let loaded = store
            .get_tc_token("user@lid")
            .await
            .expect("get failed")
            .expect("should exist");

        assert_eq!(loaded.token, vec![1, 2, 3, 4, 5]);
        assert_eq!(loaded.token_timestamp, 1707000000);
        assert_eq!(loaded.sender_timestamp, Some(1707000100));
    }

    #[tokio::test]
    async fn test_tc_token_upsert() {
        let store = create_test_store().await;

        let entry1 = TcTokenEntry {
            token: vec![1, 2, 3],
            token_timestamp: 1000,
            sender_timestamp: None,
        };
        store.put_tc_token("user@lid", &entry1).await.unwrap();

        let entry2 = TcTokenEntry {
            token: vec![4, 5, 6],
            token_timestamp: 2000,
            sender_timestamp: Some(1500),
        };
        store.put_tc_token("user@lid", &entry2).await.unwrap();

        let loaded = store.get_tc_token("user@lid").await.unwrap().unwrap();
        assert_eq!(loaded.token, vec![4, 5, 6]);
        assert_eq!(loaded.token_timestamp, 2000);
        assert_eq!(loaded.sender_timestamp, Some(1500));
    }

    #[tokio::test]
    async fn test_tc_token_delete() {
        let store = create_test_store().await;

        let entry = TcTokenEntry {
            token: vec![1, 2, 3],
            token_timestamp: 1000,
            sender_timestamp: None,
        };
        store.put_tc_token("user@lid", &entry).await.unwrap();
        store.delete_tc_token("user@lid").await.unwrap();

        let result = store.get_tc_token("user@lid").await.unwrap();
        assert!(result.is_none());
    }

    #[tokio::test]
    async fn test_tc_token_get_all_jids() {
        let store = create_test_store().await;

        let entry = TcTokenEntry {
            token: vec![1],
            token_timestamp: 1000,
            sender_timestamp: None,
        };
        store.put_tc_token("user1@lid", &entry).await.unwrap();
        store.put_tc_token("user2@lid", &entry).await.unwrap();
        store.put_tc_token("user3@lid", &entry).await.unwrap();

        let mut jids = store.get_all_tc_token_jids().await.unwrap();
        jids.sort();
        assert_eq!(jids, vec!["user1@lid", "user2@lid", "user3@lid"]);
    }

    #[tokio::test]
    async fn test_tc_token_delete_expired() {
        let store = create_test_store().await;

        let old = TcTokenEntry {
            token: vec![1],
            token_timestamp: 1000,
            sender_timestamp: None,
        };
        let recent = TcTokenEntry {
            token: vec![2],
            token_timestamp: 5000,
            sender_timestamp: None,
        };
        store.put_tc_token("old@lid", &old).await.unwrap();
        store.put_tc_token("recent@lid", &recent).await.unwrap();

        let deleted = store.delete_expired_tc_tokens(3000).await.unwrap();
        assert_eq!(deleted, 1);

        assert!(store.get_tc_token("old@lid").await.unwrap().is_none());
        assert!(store.get_tc_token("recent@lid").await.unwrap().is_some());
    }

    #[tokio::test]
    async fn test_tc_token_get_nonexistent() {
        let store = create_test_store().await;
        let result = store.get_tc_token("nonexistent@lid").await.unwrap();
        assert!(result.is_none());
    }

    #[tokio::test]
    async fn test_sender_key_devices_different_groups() {
        let store = create_test_store().await;

        let group1 = "group1@g.us";
        let group2 = "group2@g.us";

        store
            .set_sender_key_status(group1, &[("user:5@lid", true)])
            .await
            .expect("set failed");

        let g1 = store.get_sender_key_devices(group1).await.unwrap();
        assert_eq!(g1.len(), 1);

        let g2 = store.get_sender_key_devices(group2).await.unwrap();
        assert!(g2.is_empty());
    }

    #[tokio::test]
    async fn test_create_new_device_uses_configured_device_id() {
        use portable_atomic::AtomicU64;
        use std::sync::atomic::Ordering;
        static COUNTER: AtomicU64 = AtomicU64::new(100);
        let id = COUNTER.fetch_add(1, Ordering::Relaxed);
        let db_name = format!(
            "file:memdb_devid_{}_{}?mode=memory&cache=shared",
            std::process::id(),
            id
        );

        let device_id = 42;
        let store = SqliteStore::new_for_device(&db_name, device_id)
            .await
            .expect("Failed to create test store");

        assert!(!store.device_exists(device_id).await.unwrap());
        let returned_id = store.create_new_device().await.unwrap();
        assert_eq!(returned_id, device_id);
        assert!(store.device_exists(device_id).await.unwrap());

        // Row 1 should NOT exist (would if auto-increment was used)
        if device_id != 1 {
            assert!(!store.device_exists(1).await.unwrap());
        }

        let loaded = store.load_device_data_for_device(device_id).await.unwrap();
        assert!(
            loaded.is_some(),
            "device data should be loadable by configured id"
        );
    }

    /// mark_prekeys_uploaded must be UPDATE-only: a row deleted between the
    /// upload snapshot and the mark (consumed one-time key) stays deleted.
    #[tokio::test]
    async fn mark_prekeys_uploaded_never_resurrects_deleted_rows() {
        let store = create_test_store().await;
        store
            .store_prekey(1, b"record-1", false)
            .await
            .expect("store");
        store
            .store_prekey(2, b"record-2", false)
            .await
            .expect("store");
        store.remove_prekey(1).await.expect("consume");

        store
            .mark_prekeys_uploaded(&[1, 2])
            .await
            .expect("mark uploaded");

        let gone = store.load_prekey(1).await.expect("load");
        assert!(gone.is_none(), "consumed key must not be resurrected");
        let live = store.load_prekey(2).await.expect("load");
        assert!(live.is_some(), "live key still present");
    }

    /// Round-trips the prekey watermarks through the SQLite schema: save with
    /// both counters set, reopen on the same db, load and compare. Exercises
    /// the `2026-06-10-000000_add_first_unupload_pk_id` migration and the
    /// column mapping in both upsert paths.
    #[tokio::test]
    async fn test_prekey_watermarks_survive_save_load_roundtrip() {
        use portable_atomic::AtomicU64;
        use std::sync::atomic::Ordering;

        static COUNTER: AtomicU64 = AtomicU64::new(300);
        let id = COUNTER.fetch_add(1, Ordering::Relaxed);
        let db_name = format!(
            "file:memdb_pkwatermark_{}_{}?mode=memory&cache=shared",
            std::process::id(),
            id
        );

        let device_id = 9;
        let _writer = SqliteStore::new_for_device(&db_name, device_id)
            .await
            .expect("create store");
        _writer.create_new_device().await.expect("create device");

        let mut device = _writer
            .load_device_data_for_device(device_id)
            .await
            .expect("load")
            .expect("device should exist after create");
        assert_eq!(
            device.first_unupload_pre_key_id, 0,
            "fresh device starts with the watermark unset"
        );
        device.next_pre_key_id = 913;
        device.first_unupload_pre_key_id = 101;
        _writer
            .save_device_data_for_device(device_id, &device)
            .await
            .expect("save with watermarks");

        let store = SqliteStore::new_for_device(&db_name, device_id)
            .await
            .expect("reopen store");
        let loaded = store
            .load_device_data_for_device(device_id)
            .await
            .expect("load")
            .expect("device should exist after reopen");
        assert_eq!(loaded.next_pre_key_id, 913);
        assert_eq!(
            loaded.first_unupload_pre_key_id, 101,
            "first_unupload_pre_key_id must survive a save/load roundtrip"
        );
    }

    /// Round-trips a `CachedServerCertChain` through the SQLite schema:
    /// save → close store → reopen on the same db_name → load. Exercises
    /// the `2026-04-26-000000_add_server_cert_chain` migration plus the
    /// protobuf encode/decode path in `save_device_data_for_device` /
    /// `load_device_data_for_device` (the part that the in-memory backend
    /// integration tests don't reach).
    #[tokio::test]
    async fn test_server_cert_chain_survives_save_load_roundtrip() {
        use portable_atomic::AtomicU64;
        use std::sync::atomic::Ordering;
        use wacore::store::device::{CachedNoiseCert, CachedServerCertChain};

        static COUNTER: AtomicU64 = AtomicU64::new(200);
        let id = COUNTER.fetch_add(1, Ordering::Relaxed);
        // shared-cache so a second SqliteStore opened on the same name
        // sees the same on-disk state — the closest we can get to a real
        // process restart inside a single test run.
        let db_name = format!(
            "file:memdb_certchain_{}_{}?mode=memory&cache=shared",
            std::process::id(),
            id
        );

        let device_id = 7;
        let chain = CachedServerCertChain {
            intermediate: CachedNoiseCert {
                key: [0xAB; 32],
                not_before: 1_700_000_000,
                not_after: 1_900_000_000,
            },
            leaf: CachedNoiseCert {
                key: [0xCD; 32],
                not_before: 1_700_000_500,
                not_after: 1_899_999_500,
            },
        };

        // First store: create + populate. Keep it alive until after the
        // second store opens — `cache=shared` only persists the in-memory
        // database while at least one connection is open. Dropping the
        // first store would also drop the schema before the second can
        // see it.
        let _writer = SqliteStore::new_for_device(&db_name, device_id)
            .await
            .expect("create store");
        _writer.create_new_device().await.expect("create device");

        let mut device = _writer
            .load_device_data_for_device(device_id)
            .await
            .expect("load")
            .expect("device should exist after create");
        device.server_cert_chain = Some(chain.clone());
        _writer
            .save_device_data_for_device(device_id, &device)
            .await
            .expect("save with cert chain");

        // Second store on the SAME shared-cache db: this exercises the
        // exact path a fresh-process load would take — schema migration
        // already applied, BLOB column present, and the protobuf-encoded
        // chain decoded by the load path.
        let store = SqliteStore::new_for_device(&db_name, device_id)
            .await
            .expect("reopen store");
        let loaded = store
            .load_device_data_for_device(device_id)
            .await
            .expect("load")
            .expect("device should exist after reopen");
        assert_eq!(
            loaded.server_cert_chain.as_ref(),
            Some(&chain),
            "server_cert_chain must survive a save/load roundtrip"
        );

        // Sanity: clearing the chain and saving leaves the column as NULL,
        // not as an empty serialized struct.
        let mut device = loaded;
        device.server_cert_chain = None;
        store
            .save_device_data_for_device(device_id, &device)
            .await
            .expect("save with cleared cert chain");

        let reloaded = store
            .load_device_data_for_device(device_id)
            .await
            .expect("reload")
            .expect("device should exist");
        assert!(
            reloaded.server_cert_chain.is_none(),
            "cleared chain must round-trip as None"
        );
    }

    // The migration strategy is self-healing with NO migration: rows written by the
    // old `bincode` codec can't decode as the new protobuf wire format, so the store
    // must read them back as ABSENT (never an error) -- then the sync path re-requests
    // the key / re-syncs the collection, and the protobuf setters overwrite the row.
    #[tokio::test]
    async fn legacy_bincode_blobs_self_heal_then_overwrite() {
        use diesel::{ExpressionMethods, RunQueryDsl, sql_query};
        use wacore::appstate::hash::HashState;
        use wacore::store::traits::AppStateSyncKey;

        // Exact bytes `bincode` 2.0.1 (config::standard, via serde) produced for these
        // domain structs before the migration, captured with the real codec. They must
        // not parse as the protobuf wire format.
        // AppStateSyncKey { key_data: [0x11;32], fingerprint: [aa bb cc dd], timestamp: 1_700_000_000 }.
        let legacy_sync_key = {
            let mut v = vec![0x20u8]; // bincode varint len 32
            v.extend([0x11u8; 32]);
            v.extend([0x04, 0xaa, 0xbb, 0xcc, 0xdd, 0xfc, 0x00, 0xe2, 0xa7, 0xca]);
            v
        };
        // HashState { version: 7, hash: [de ad 00..00 be], index_value_map: {} }.
        let legacy_hash_state = {
            let mut v = vec![0x07u8]; // version varint 7
            v.push(0xde);
            v.push(0xad);
            v.extend([0u8; 125]);
            v.push(0xbe);
            v.push(0x00); // empty map
            v
        };

        let store = create_test_store().await;
        let device_id = store.device_id;

        // Insert the legacy rows directly (bypassing the protobuf setters), exactly as
        // an upgraded DB would already hold them.
        let key_id = b"legacy-key".to_vec();
        {
            let kid = key_id.clone();
            let blob = legacy_sync_key.clone();
            store
                .with_retry("insert_legacy_key", move || {
                    let kid = kid.clone();
                    let blob = blob.clone();
                    Box::new(move |conn| {
                        diesel::insert_into(app_state_keys::table)
                            .values((
                                app_state_keys::key_id.eq(kid),
                                app_state_keys::key_data.eq(blob),
                                app_state_keys::device_id.eq(device_id),
                            ))
                            .execute(conn)
                            .map(|_| ())
                    })
                })
                .await
                .expect("insert legacy key row");
        }
        let name = "critical_block";
        {
            let blob = legacy_hash_state.clone();
            store
                .with_retry("insert_legacy_version", move || {
                    let blob = blob.clone();
                    Box::new(move |conn| {
                        diesel::insert_into(app_state_versions::table)
                            .values((
                                app_state_versions::name.eq(name),
                                app_state_versions::state_data.eq(blob),
                                app_state_versions::device_id.eq(device_id),
                            ))
                            .execute(conn)
                            .map(|_| ())
                    })
                })
                .await
                .expect("insert legacy version row");
        }

        // Self-heal: a legacy bincode row reads back as absent / default, NOT an error,
        // and never as a partially-decoded protobuf with garbage material.
        assert!(
            store
                .get_app_state_sync_key_for_device(&key_id, device_id)
                .await
                .expect("legacy sync-key blob must not surface a decode error")
                .is_none(),
            "a legacy bincode sync-key row must read back as absent"
        );
        assert_eq!(
            store
                .get_app_state_version_for_device(name, device_id)
                .await
                .expect("legacy version blob must not surface a decode error")
                .version,
            0,
            "a legacy bincode version row must reset to default (re-sync from 0)"
        );

        // And the protobuf setters overwrite the healed rows: a re-shared key and a
        // fresh version persist and read back correctly afterwards.
        store
            .set_app_state_sync_key_for_device(
                &key_id,
                AppStateSyncKey {
                    key_data: vec![7u8; 32],
                    fingerprint: vec![1, 2, 3],
                    timestamp: 99,
                },
                device_id,
            )
            .await
            .expect("overwrite key");
        let healed_key = store
            .get_app_state_sync_key_for_device(&key_id, device_id)
            .await
            .expect("get key")
            .expect("re-shared key must persist over the legacy row");
        assert_eq!(healed_key.key_data, vec![7u8; 32]);
        assert_eq!(healed_key.timestamp, 99);

        store
            .set_app_state_version_for_device(
                name,
                HashState {
                    version: 5,
                    ..HashState::default()
                },
                device_id,
            )
            .await
            .expect("overwrite version");
        assert_eq!(
            store
                .get_app_state_version_for_device(name, device_id)
                .await
                .expect("get version")
                .version,
            5,
            "a re-synced version must persist over the legacy row"
        );

        // Genuine corruption (not a clean bincode blob) is handled the same way.
        store
            .with_retry("corrupt_key", || {
                Box::new(|conn| {
                    sql_query("UPDATE app_state_keys SET key_data = X'00ff00ff'")
                        .execute(conn)
                        .map(|_| ())
                })
            })
            .await
            .expect("corrupt key blob");
        assert!(
            store
                .get_app_state_sync_key_for_device(&key_id, device_id)
                .await
                .expect("corrupt key blob must not error")
                .is_none(),
            "an arbitrarily corrupt sync-key blob must also read back as absent"
        );
    }

    // Outbound mutations (chat actions) encrypt with the latest sync key, so the
    // latest-key selection must skip a legacy bincode row even when it sorts higher --
    // otherwise build_patch would later fail in get_app_state_key with KeyNotFound.
    #[tokio::test]
    async fn latest_sync_key_skips_undecodable_rows() {
        use diesel::{ExpressionMethods, RunQueryDsl};
        use wacore::store::traits::AppStateSyncKey;

        // Real bincode 2.0.1 bytes for an AppStateSyncKey -- undecodable as protobuf.
        let legacy_blob = {
            let mut v = vec![0x20u8];
            v.extend([0x11u8; 32]);
            v.extend([0x04, 0xaa, 0xbb, 0xcc, 0xdd, 0xfc, 0x00, 0xe2, 0xa7, 0xca]);
            v
        };

        let store = create_test_store().await;
        let device_id = store.device_id;

        // A valid (protobuf) key at a LOWER key_id...
        let good_id = b"key-aaa".to_vec();
        store
            .set_app_state_sync_key_for_device(
                &good_id,
                AppStateSyncKey {
                    key_data: vec![7u8; 32],
                    fingerprint: vec![1],
                    timestamp: 1,
                },
                device_id,
            )
            .await
            .unwrap();

        // ...and a stale bincode row at a lexicographically HIGHER key_id, inserted raw.
        let bad_id = b"key-zzz".to_vec();
        {
            let bid = bad_id.clone();
            let blob = legacy_blob.clone();
            store
                .with_retry("insert_stale_key", move || {
                    let bid = bid.clone();
                    let blob = blob.clone();
                    Box::new(move |conn| {
                        diesel::insert_into(app_state_keys::table)
                            .values((
                                app_state_keys::key_id.eq(bid),
                                app_state_keys::key_data.eq(blob),
                                app_state_keys::device_id.eq(device_id),
                            ))
                            .execute(conn)
                            .map(|_| ())
                    })
                })
                .await
                .unwrap();
        }

        // The higher-but-undecodable row must be skipped for the usable key.
        assert_eq!(
            store
                .get_latest_app_state_sync_key_id_for_device(device_id)
                .await
                .unwrap(),
            Some(good_id),
            "latest-key selection must skip undecodable bincode rows"
        );
    }

    #[tokio::test]
    async fn group_metadata_round_trip_sqlite() {
        use wacore::store::traits::ProtocolStore;
        let store = create_test_store().await;
        let jid = "120363000000000001@g.us";

        assert!(store.get_group_metadata(jid).await.unwrap().is_none());

        store.put_group_metadata(jid, b"blob-v1").await.unwrap();
        assert_eq!(
            store.get_group_metadata(jid).await.unwrap().as_deref(),
            Some(&b"blob-v1"[..])
        );

        // Upsert overwrites the prior blob.
        store.put_group_metadata(jid, b"blob-v2").await.unwrap();
        assert_eq!(
            store.get_group_metadata(jid).await.unwrap().as_deref(),
            Some(&b"blob-v2"[..])
        );

        // Delete drops the blob so the next query re-fetches in full.
        store.delete_group_metadata(jid).await.unwrap();
        assert!(store.get_group_metadata(jid).await.unwrap().is_none());
    }

    #[tokio::test]
    async fn msg_secret_round_trip_sqlite() {
        let store = create_test_store().await;
        let secret = [0xABu8; 32];
        store
            .put_msg_secret("12345@s.whatsapp.net", "9999@lid", "MID1", &secret)
            .await
            .expect("put");
        let got = store
            .get_msg_secret("12345@s.whatsapp.net", "9999@lid", "MID1")
            .await
            .expect("get")
            .expect("must exist");
        assert_eq!(got, secret.to_vec());
    }

    #[tokio::test]
    async fn msg_secret_miss_returns_none_sqlite() {
        let store = create_test_store().await;
        assert!(
            store
                .get_msg_secret("any@s.whatsapp.net", "any@lid", "NOPE")
                .await
                .expect("get")
                .is_none()
        );
    }

    #[tokio::test]
    async fn msg_secret_upsert_replaces_secret() {
        let store = create_test_store().await;
        store
            .put_msg_secret("c", "s", "M", &[1u8; 32])
            .await
            .expect("put 1");
        store
            .put_msg_secret("c", "s", "M", &[9u8; 32])
            .await
            .expect("put 2");
        let got = store.get_msg_secret("c", "s", "M").await.unwrap().unwrap();
        assert_eq!(got, vec![9u8; 32], "ON CONFLICT must overwrite");
    }

    #[tokio::test]
    async fn msg_secret_scoped_by_three_columns() {
        let store = create_test_store().await;
        store
            .put_msg_secret("c1", "s1", "M1", &[1u8; 32])
            .await
            .unwrap();
        store
            .put_msg_secret("c1", "s1", "M2", &[2u8; 32])
            .await
            .unwrap();
        store
            .put_msg_secret("c1", "s2", "M1", &[3u8; 32])
            .await
            .unwrap();
        store
            .put_msg_secret("c2", "s1", "M1", &[4u8; 32])
            .await
            .unwrap();

        for (chat, sender, msg_id, expected) in [
            ("c1", "s1", "M1", 1u8),
            ("c1", "s1", "M2", 2),
            ("c1", "s2", "M1", 3),
            ("c2", "s1", "M1", 4),
        ] {
            let got = store
                .get_msg_secret(chat, sender, msg_id)
                .await
                .unwrap()
                .unwrap_or_else(|| panic!("missing ({chat},{sender},{msg_id})"));
            assert_eq!(got, vec![expected; 32]);
        }
    }

    #[tokio::test]
    async fn msg_secret_batch_upserts_in_one_call() {
        let store = create_test_store().await;
        let stored = store
            .put_msg_secrets(vec![
                MsgSecretEntry {
                    chat: "c".into(),
                    sender: "s".into(),
                    msg_id: "M1".into(),
                    secret: vec![1u8; 32],
                    expires_at: 0,
                    message_ts: 0,
                },
                MsgSecretEntry {
                    chat: "c".into(),
                    sender: "s".into(),
                    msg_id: "M2".into(),
                    secret: vec![2u8; 32],
                    expires_at: 0,
                    message_ts: 0,
                },
                MsgSecretEntry {
                    chat: "c".into(),
                    sender: "s".into(),
                    msg_id: "M1".into(),
                    secret: vec![9u8; 32],
                    expires_at: 0,
                    message_ts: 0,
                },
            ])
            .await
            .unwrap();

        assert_eq!(stored, 3);
        assert_eq!(
            store.get_msg_secret("c", "s", "M1").await.unwrap().unwrap(),
            vec![9u8; 32]
        );
        assert_eq!(
            store.get_msg_secret("c", "s", "M2").await.unwrap().unwrap(),
            vec![2u8; 32]
        );
    }

    #[tokio::test]
    async fn delete_expired_msg_secrets_deletes_only_passed_deadlines() {
        let store = create_test_store().await;
        let now = wacore::time::now_secs();
        store
            .put_msg_secrets(vec![
                MsgSecretEntry {
                    chat: "c".into(),
                    sender: "s".into(),
                    msg_id: "NEVER".into(),
                    secret: vec![1u8; 32],
                    expires_at: 0,
                    message_ts: 0,
                },
                MsgSecretEntry {
                    chat: "c".into(),
                    sender: "s".into(),
                    msg_id: "FUTURE".into(),
                    secret: vec![2u8; 32],
                    expires_at: now + 86_400,
                    message_ts: 0,
                },
                MsgSecretEntry {
                    chat: "c".into(),
                    sender: "s".into(),
                    msg_id: "PAST".into(),
                    secret: vec![3u8; 32],
                    expires_at: now - 86_400,
                    message_ts: 0,
                },
            ])
            .await
            .unwrap();

        let removed = store.delete_expired_msg_secrets(now).await.unwrap();
        assert_eq!(
            removed, 1,
            "only the row whose deadline has passed is deleted"
        );
        assert!(
            store
                .get_msg_secret("c", "s", "NEVER")
                .await
                .unwrap()
                .is_some(),
            "expires_at = 0 never expires"
        );
        assert!(
            store
                .get_msg_secret("c", "s", "FUTURE")
                .await
                .unwrap()
                .is_some(),
            "a future deadline survives"
        );
        assert!(
            store
                .get_msg_secret("c", "s", "PAST")
                .await
                .unwrap()
                .is_none(),
            "a passed deadline is pruned"
        );
    }

    #[tokio::test]
    async fn put_msg_secrets_keeps_later_deadline_on_conflict() {
        let store = create_test_store().await;
        let now = wacore::time::now_secs();
        // First write a finite deadline, then a re-persist with an EARLIER one:
        // the window must not shrink.
        store
            .put_msg_secrets(vec![MsgSecretEntry {
                chat: "c".into(),
                sender: "s".into(),
                msg_id: "M".into(),
                secret: vec![1u8; 32],
                expires_at: now + 90 * 86_400,
                message_ts: 0,
            }])
            .await
            .unwrap();
        store
            .put_msg_secrets(vec![MsgSecretEntry {
                chat: "c".into(),
                sender: "s".into(),
                msg_id: "M".into(),
                secret: vec![1u8; 32],
                expires_at: now + 30 * 86_400,
                message_ts: 0,
            }])
            .await
            .unwrap();
        // The 90-day deadline must remain: a cutoff at now+60d deletes nothing.
        let removed = store
            .delete_expired_msg_secrets(now + 60 * 86_400)
            .await
            .unwrap();
        assert_eq!(removed, 0, "conflict must keep the later (90d) deadline");

        // A never-expire (0) write must override any finite deadline.
        store
            .put_msg_secret("c", "s", "M", &[1u8; 32])
            .await
            .unwrap();
        let removed = store
            .delete_expired_msg_secrets(now + 200 * 86_400)
            .await
            .unwrap();
        assert_eq!(removed, 0, "a 0 (never) deadline wins over any finite one");
    }

    #[tokio::test]
    async fn get_msg_secret_with_ts_round_trips_and_keeps_parent_ts() {
        let store = create_test_store().await;
        let parent_ts = 1_700_000_000i64;
        store
            .put_msg_secrets(vec![MsgSecretEntry {
                chat: "c".into(),
                sender: "s".into(),
                msg_id: "M".into(),
                secret: vec![5u8; 32],
                expires_at: 0,
                message_ts: parent_ts,
            }])
            .await
            .unwrap();
        assert_eq!(
            store.get_msg_secret_with_ts("c", "s", "M").await.unwrap(),
            Some((vec![5u8; 32], parent_ts))
        );

        // A later write with an unknown ts (0) must not clobber the known one.
        store
            .put_msg_secret("c", "s", "M", &[5u8; 32])
            .await
            .unwrap();
        assert_eq!(
            store.get_msg_secret_with_ts("c", "s", "M").await.unwrap(),
            Some((vec![5u8; 32], parent_ts)),
            "message_ts (immutable parent time) must survive a 0-ts redelivery"
        );

        // Absent row → None.
        assert_eq!(
            store
                .get_msg_secret_with_ts("c", "s", "MISSING")
                .await
                .unwrap(),
            None
        );
    }

    /// Multi-account isolation: same DB, different device_id rows must not
    /// collide on the same logical key.
    #[tokio::test]
    async fn msg_secret_isolated_per_device_id() {
        use portable_atomic::AtomicU64;
        use std::sync::atomic::Ordering;
        static COUNTER: AtomicU64 = AtomicU64::new(0);
        let id = COUNTER.fetch_add(1, Ordering::Relaxed);
        let shared_url = format!(
            "file:memdb_msgsecret_iso_{}_{}?mode=memory&cache=shared",
            std::process::id(),
            id
        );
        let store_a = SqliteStore::new_for_device(&shared_url, 1)
            .await
            .expect("store_a");
        let store_b = SqliteStore::new_for_device(&shared_url, 2)
            .await
            .expect("store_b");

        store_a
            .put_msg_secret("c", "s", "M", &[7u8; 32])
            .await
            .unwrap();
        assert!(
            store_b
                .get_msg_secret("c", "s", "M")
                .await
                .unwrap()
                .is_none(),
            "same DB, different device_id must not see each other's secrets"
        );
        assert_eq!(
            store_a
                .get_msg_secret("c", "s", "M")
                .await
                .unwrap()
                .unwrap(),
            vec![7u8; 32],
            "device_a still sees its own write"
        );
    }
}
