# Extension-safe media, business and cache configuration

The nine configuration DTOs below are now `#[non_exhaustive]`. This intentionally
breaks external struct literals, including literals using `..Default::default()`.
Public fields remain readable and writable; patterns must include `..`.

These inputs use fluent `with_*` methods directly on the existing types. No
additional feature or separate DTO family is needed. `Default` remains a valid
starting configuration; `CacheEntryConfig::new(timeout, capacity)` requires both
settings explicitly instead of inventing a default capacity.

## Replace literals with construction

```rust
use whatsapp_rust::media::{AudioOptions, DocumentOptions, ImageOptions, VideoOptions};

let image = ImageOptions::default().with_caption("hi");
let video = VideoOptions::default()
    .with_caption("clip")
    .with_duration_seconds(12)
    .with_gif_playback(true);
let document = DocumentOptions::default()
    .with_file_name("manual.pdf")
    .with_page_count(3);
let audio = AudioOptions::default().with_ptt(true).with_duration_seconds(5);
```

All media fields have corresponding `with_*` methods. String setters accept
`impl Into<String>`, context setters accept `Box<wa::ContextInfo>`, and optional
fields become `Some(value)`. To clear an optional field, assign `None` directly.
The required upload receipt remains a separate argument to each message builder;
these options do not fabricate upload or encryption metadata.

```rust
use whatsapp_rust::{CatalogOptions, CollectionOptions};

let catalog = CatalogOptions::default()
    .with_limit(20)
    .with_after("opaque cursor")
    .with_image_dimensions(80, 60)
    .with_allow_shop_source(false);
let collections = CollectionOptions::default()
    .with_collection_limit(2)
    .with_item_limit(4)
    .with_image_dimensions(100, 120);
```

Both paging configurations provide `with_after` for a returned cursor. Dimensions
and limits retain their existing meaning and defaults; construction does not add
new validation or network behavior.

```rust
use whatsapp_rust::{CacheConfig, CacheEntryConfig};

let mut config = CacheConfig::default()
    .with_group_cache(CacheEntryConfig::new(None, 1_000))
    .with_device_registry_cache(CacheEntryConfig::new(None, 5_000));
config.chat_lanes_capacity = 2_000;
let CacheConfig { group_cache, .. } = &config;
assert_eq!(group_cache.capacity, 1_000);
```

`CacheConfig` also provides `with_lid_pn_cache`, `with_recent_messages` and
`with_cache_stores`. Other advanced settings can be assigned after default
construction. `CacheStores::default()` keeps all caches local; its
`with_group_cache`, `with_device_registry_cache` and `with_lid_pn_cache` accept
`Arc<dyn CacheStore>`. `CacheStores::all(store)` continues to select one shared
backend for all pluggable caches. Coordination caches remain local.

Host traits are unchanged. In particular, external `CacheStore` implementations
remain supported on native and WASM. Existing business result enums already use
`#[non_exhaustive]`: keep a wildcard arm and preserve unknown wire strings in
`ProductAvailability::Other(raw)`.

## Independent contract coverage

`tests/fixtures/media_cache_consumer` is a standalone workspace depending on the
root crate with default features disabled. It constructs every assigned DTO,
reads public fields, uses open patterns, implements a host mock cache and matches
extensible business enums. Its native negative test checks each removed literal
individually for E0639 and an exhaustive enum match for E0004, including the
expected source file/type and absence of unrelated compiler errors.

```text
cargo test --locked --manifest-path tests/fixtures/media_cache_consumer/Cargo.toml
RUSTFLAGS='' cargo +1.94.0 test --locked --manifest-path tests/fixtures/media_cache_consumer/Cargo.toml
cargo check --locked --manifest-path tests/fixtures/media_cache_consumer/Cargo.toml --target wasm32-unknown-unknown --lib
```

Do not run this fixture with `--all-features`: its `old-*-literal` and
`exhaustive-product-availability` features are intentionally invalid modes.
WASM checks compile the host trait and construction functions; subprocess-based
negative checks run on native only.
