use parking_lot::{Condvar, Mutex};
use std::sync::Arc;

pub fn warm() {
    let shared = Arc::new((Mutex::new((0usize, false)), Condvar::new()));
    std::thread::scope(|scope| {
        // The locked parking_lot_core retains every grown global table. Enroll
        // waiters in order so scheduling cannot skip an intermediate capacity.
        for registered in 1..=32 {
            let worker = Arc::clone(&shared);
            scope.spawn(move || {
                let (lock, changed) = &*worker;
                let mut state = lock.lock();
                state.0 += 1;
                changed.notify_all();
                while !state.1 {
                    changed.wait(&mut state);
                }
            });
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
    });
}

#[cfg(test)]
mod tests {
    #[test]
    fn repeating_warmup_does_not_grow_retained_process_metadata() {
        super::warm();
        let before = crate::allocator::live();
        super::warm();
        assert_eq!(crate::allocator::live(), before);
    }
}
