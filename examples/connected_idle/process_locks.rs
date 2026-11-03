use std::sync::Arc;

macro_rules! warm_version {
    ($parking_lot:ident) => {{
        use $parking_lot::{Condvar, Mutex};
        let shared = Arc::new((Mutex::new((0usize, false)), Condvar::new()));
        std::thread::scope(|scope| {
            let mut workers = Vec::with_capacity(32);
            // The locked parking_lot_core retains every grown global table. Enroll
            // waiters in order so scheduling cannot skip an intermediate capacity.
            for registered in 1..=32 {
                let worker = Arc::clone(&shared);
                workers.push(scope.spawn(move || {
                    let (lock, changed) = &*worker;
                    let mut state = lock.lock();
                    state.0 += 1;
                    changed.notify_all();
                    while !state.1 {
                        changed.wait(&mut state);
                    }
                }));
                let (lock, changed) = &*shared;
                let mut state = lock.lock();
                while state.0 != registered {
                    changed.wait(&mut state);
                }
            }
            let (lock, changed) = &*shared;
            let mut state = lock.lock();
            state.1 = true;
            changed.notify_all();
            drop(state);
            // Explicit joins finish OS-thread TLS teardown before the heap baseline.
            for worker in workers {
                worker.join().expect("lock warmup worker");
            }
        });
    }};
}

pub fn warm() {
    // Both incompatible versions retain independent process-global tables.
    warm_version!(parking_lot_012);
    warm_version!(parking_lot);
}

#[cfg(test)]
mod tests {
    #[test]
    fn repeating_warmup_does_not_grow_retained_process_metadata() {
        if crate::allocator::run_in_child(
            "process_locks::tests::repeating_warmup_does_not_grow_retained_process_metadata",
        ) {
            return;
        }
        super::warm();
        let before = crate::allocator::live();
        super::warm();
        assert_eq!(crate::allocator::live(), before);
    }
}
