use portable_atomic::{AtomicU64, Ordering};

pub(super) fn next() -> String {
    static PROBE_SEQ: AtomicU64 = AtomicU64::new(0);
    format!(
        "__wa_durability_probe_{}_{}__",
        rand::random::<u128>(),
        PROBE_SEQ.fetch_add(1, Ordering::Relaxed)
    )
}
