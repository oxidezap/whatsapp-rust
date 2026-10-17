use std::io::{self, Cursor, Seek, SeekFrom, Write};
use std::sync::{Arc, Mutex};

use super::{DownloadPolicyError, DownloadWriter};

#[derive(Clone)]
pub(crate) struct MemoryBudget(Arc<BudgetState>);

struct BudgetState {
    limit: usize,
    used: Mutex<usize>,
}

impl MemoryBudget {
    pub(crate) fn new(limit: usize) -> Self {
        Self(Arc::new(BudgetState {
            limit,
            used: Mutex::new(0),
        }))
    }

    #[cfg(test)]
    pub(crate) fn used(&self) -> usize {
        *self.0.used.lock().unwrap()
    }

    fn limit(&self) -> usize {
        self.0.limit
    }
}

pub(super) struct MemoryReservation {
    budget: MemoryBudget,
    bytes: usize,
}

impl MemoryReservation {
    fn resize(&mut self, required: usize, preferred: usize) -> Result<usize, DownloadPolicyError> {
        // These bytes already belong to this lease, regardless of other reservations.
        if required <= self.bytes && preferred == self.bytes {
            return Ok(self.bytes);
        }
        let mut used = self
            .budget
            .0
            .used
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let available = self.budget.0.limit - (*used - self.bytes);
        // Waiting can deadlock against completed blobs retained by this batch.
        if required > available {
            return Err(DownloadPolicyError::MemoryBudgetExceeded {
                limit: self.budget.0.limit,
            });
        }
        let capacity = preferred.min(available).max(required);
        *used = *used - self.bytes + capacity;
        self.bytes = capacity;
        Ok(capacity)
    }
}

impl Drop for MemoryReservation {
    fn drop(&mut self) {
        let mut used = self
            .budget
            .0
            .used
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        *used -= self.bytes;
    }
}

#[derive(Debug, thiserror::Error)]
#[error("{0}")]
pub(super) struct MemoryPolicyCause(#[source] pub(super) DownloadPolicyError);

fn policy_io(error: DownloadPolicyError) -> io::Error {
    io::Error::other(MemoryPolicyCause(error))
}

/// Starts without a payload allocation. Small budgets stay lazy; large admitted
/// buffers may preallocate on the caller. Reservations follow the allocation
/// into the prefetch map, including while other jobs run.
pub(crate) struct MemoryWriter {
    cursor: Cursor<Vec<u8>>,
    initial_capacity: usize,
    reservation: Option<MemoryReservation>,
}

impl MemoryWriter {
    pub(super) fn new(initial_capacity: usize, budget: Option<MemoryBudget>) -> Self {
        Self {
            cursor: Cursor::new(Vec::new()),
            initial_capacity,
            reservation: budget.map(|budget| MemoryReservation { budget, bytes: 0 }),
        }
    }

    pub(crate) fn bounded(
        declared: u64,
        budget: MemoryBudget,
    ) -> Result<Self, DownloadPolicyError> {
        let size =
            usize::try_from(declared).map_err(|_| DownloadPolicyError::MemoryBudgetExceeded {
                limit: budget.limit(),
            })?;
        let mut writer = Self::new(0, Some(budget));
        if let Some(reservation) = &mut writer.reservation {
            reservation.resize(size, size)?;
        }
        Ok(writer)
    }

    pub(crate) fn into_bytes(self) -> bytes::Bytes {
        // Bytes clones and slices must retain the lease with the allocation.
        let (buffer, reservation) = self.into_parts();
        bytes::Bytes::from_owner(MemoryBody {
            buffer,
            _reservation: reservation,
        })
    }

    pub(super) fn into_parts(mut self) -> (Vec<u8>, Option<MemoryReservation>) {
        if let Some(reservation) = &mut self.reservation {
            let actual = self.cursor.get_ref().capacity();
            assert!(actual <= reservation.bytes);
            if actual < reservation.bytes {
                let mut used = reservation
                    .budget
                    .0
                    .used
                    .lock()
                    .unwrap_or_else(std::sync::PoisonError::into_inner);
                *used -= reservation.bytes - actual;
                reservation.bytes = actual;
            }
        }
        (self.cursor.into_inner(), self.reservation)
    }

    // Large admitted blobs can allocate on the async caller, where the
    // retained bytes will also be published/dropped. Small embedded budgets
    // remain lazy so network/foreground admission can release transient RAM.
    pub(crate) fn preallocate_large(&mut self) -> Result<(), DownloadPolicyError> {
        if let Some(reservation) = &self.reservation
            && reservation.bytes >= 64 * 1024
        {
            self.reserve(reservation.bytes)?;
        }
        Ok(())
    }

    fn reserve(&mut self, required: usize) -> Result<(), DownloadPolicyError> {
        let current = self.cursor.get_ref().capacity();
        if required <= current {
            return Ok(());
        }
        let preferred = if let Some(reservation) = &self.reservation {
            required
                .max(current.saturating_mul(2))
                .max(reservation.bytes)
        } else {
            required
                .max(current.saturating_mul(2))
                .max(self.initial_capacity)
        };
        let target = if let Some(reservation) = &mut self.reservation {
            reservation.resize(required, preferred)?
        } else {
            preferred
        };
        let buffer = self.cursor.get_mut();
        if buffer.try_reserve_exact(target - buffer.len()).is_err() {
            if let Some(reservation) = &mut self.reservation {
                reservation.resize(current, current)?;
            }
            return Err(DownloadPolicyError::AllocationFailed);
        }
        if let Some(reservation) = &mut self.reservation {
            let actual = buffer.capacity().max(reservation.bytes);
            if let Err(error) = reservation.resize(actual, actual) {
                // An allocator may supply excess capacity. Never publish an
                // allocation that cannot be charged to the shared budget.
                *buffer = Vec::new();
                reservation.resize(0, 0)?;
                return Err(error);
            }
        }
        Ok(())
    }
}

struct MemoryBody {
    buffer: Vec<u8>,
    _reservation: Option<MemoryReservation>,
}

impl AsRef<[u8]> for MemoryBody {
    fn as_ref(&self) -> &[u8] {
        &self.buffer
    }
}

impl Write for MemoryWriter {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        if bytes.is_empty() {
            return Ok(0);
        }
        let position = usize::try_from(self.cursor.position())
            .map_err(|_| policy_io(DownloadPolicyError::AllocationFailed))?;
        let required = position
            .checked_add(bytes.len())
            .ok_or_else(|| policy_io(DownloadPolicyError::AllocationFailed))?;
        self.reserve(required).map_err(policy_io)?;
        self.cursor.write(bytes)
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

impl Seek for MemoryWriter {
    fn seek(&mut self, position: SeekFrom) -> io::Result<u64> {
        self.cursor.seek(position)
    }
}

impl DownloadWriter for MemoryWriter {
    fn truncate(&mut self, len: u64) -> io::Result<()> {
        let len =
            usize::try_from(len).map_err(|_| policy_io(DownloadPolicyError::AllocationFailed))?;
        if len > self.cursor.get_ref().len() {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "cannot extend media buffer by truncating",
            ));
        }
        self.cursor.get_mut().truncate(len);
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn used(budget: &MemoryBudget) -> usize {
        *budget.0.used.lock().unwrap()
    }

    #[test]
    fn eager_large_buffers_stay_empty_and_keep_clone_lifetime_accounting() {
        for declared in [8192, 65536] {
            let budget = MemoryBudget::new(declared);
            let mut writer = MemoryWriter::bounded(declared as u64, budget.clone()).unwrap();
            writer.preallocate_large().unwrap();
            assert!(writer.cursor.get_ref().is_empty());
            assert_eq!(
                writer.cursor.get_ref().capacity(),
                if declared == 8192 { 0 } else { declared }
            );
            writer.write_all(b"verified payload").unwrap();
            let bytes = writer.into_bytes();
            assert_eq!(bytes.as_ref(), b"verified payload");
            let clone = bytes.clone();
            drop(bytes);
            assert_eq!(used(&budget), declared);
            assert!(MemoryWriter::bounded(1, budget.clone()).is_err());
            drop(clone);
            assert_eq!(used(&budget), 0);
        }
    }

    #[test]
    fn eager_allocation_overflow_is_fallible_and_releases_the_reservation() {
        let budget = MemoryBudget::new(usize::MAX);
        let mut writer = MemoryWriter::bounded(usize::MAX as u64, budget.clone()).unwrap();
        assert_eq!(
            writer.preallocate_large(),
            Err(DownloadPolicyError::AllocationFailed)
        );
        assert_eq!(writer.cursor.get_ref().capacity(), 0);
        assert_eq!(used(&budget), 0);
        drop(writer);
        assert_eq!(used(&budget), 0);
    }

    #[test]
    fn bytes_clones_and_slices_keep_the_allocation_charged() {
        let budget = MemoryBudget::new(32);
        let mut writer = MemoryWriter::bounded(32, budget.clone()).unwrap();
        writer.write_all(&[7; 32]).unwrap();
        let bytes = writer.into_bytes();
        let clone = bytes.clone();
        let slice = clone.slice(1..3);
        drop((bytes, clone));
        assert_eq!(slice.as_ref(), &[7; 2]);
        assert_eq!(used(&budget), 32);
        assert!(MemoryWriter::bounded(1, budget.clone()).is_err());
        drop(slice);
        assert_eq!(used(&budget), 0);
    }

    #[test]
    fn declared_sizes_are_admitted_without_allocating_bodies() {
        let budget = MemoryBudget::new(64);
        let first = MemoryWriter::bounded(32, budget.clone()).unwrap();
        let second = MemoryWriter::bounded(32, budget.clone()).unwrap();
        assert_eq!(first.cursor.get_ref().capacity(), 0);
        assert_eq!(second.cursor.get_ref().capacity(), 0);
        assert_eq!(used(&budget), 64);
        assert!(matches!(
            MemoryWriter::bounded(1, budget.clone()),
            Err(DownloadPolicyError::MemoryBudgetExceeded { limit: 64 })
        ));
        drop(first);
        let third = MemoryWriter::bounded(32, budget.clone()).unwrap();
        drop((second, third));
        assert_eq!(used(&budget), 0);
    }

    #[test]
    fn lazy_reservation_follows_completed_buffer_until_drop() {
        let budget = MemoryBudget::new(64);
        let mut writer = MemoryWriter::new(usize::MAX, Some(budget.clone()));
        assert_eq!(used(&budget), 0);
        writer.write_all(&[1; 32]).unwrap();
        let (bytes, reservation) = writer.into_parts();
        assert_eq!(bytes.len(), 32);
        assert_eq!(used(&budget), bytes.capacity());
        let mut next = MemoryWriter::new(0, Some(budget.clone()));
        next.write_all(&[2; 32]).unwrap();
        assert!(next.write_all(&[3]).is_err());
        assert_eq!(used(&budget), 64);
        drop((bytes, reservation));
        next.write_all(&[3; 32]).unwrap();
        assert_eq!(used(&budget), 64);
        drop(next);
        assert_eq!(used(&budget), 0);
    }

    #[test]
    fn oversized_and_overflow_allocations_fail_without_charging() {
        let budget = MemoryBudget::new(16);
        let mut writer = MemoryWriter::new(0, Some(budget.clone()));
        assert!(writer.write_all(&[0; 17]).is_err());
        assert_eq!(used(&budget), 0);
        assert_eq!(writer.cursor.get_ref().capacity(), 0);
        let mut writer = MemoryWriter::new(usize::MAX, None);
        let error = writer.write_all(b"x").unwrap_err();
        assert!(matches!(
            error.get_ref().unwrap().downcast_ref::<MemoryPolicyCause>(),
            Some(MemoryPolicyCause(DownloadPolicyError::AllocationFailed))
        ));
        assert_eq!(writer.cursor.get_ref().capacity(), 0);
    }

    #[test]
    fn concurrent_reservations_never_overcommit_or_wait() {
        let budget = MemoryBudget::new(64);
        let barrier = Arc::new(std::sync::Barrier::new(4));
        let handles: Vec<_> = (0..4)
            .map(|_| {
                let budget = budget.clone();
                let barrier = barrier.clone();
                std::thread::spawn(move || {
                    let mut writer = MemoryWriter::new(0, Some(budget.clone()));
                    let success = writer.write_all(&[0; 32]).is_ok();
                    barrier.wait();
                    assert_eq!(used(&budget), 64);
                    barrier.wait();
                    success
                })
            })
            .collect();
        let successes = handles
            .into_iter()
            .map(|handle| handle.join().unwrap())
            .filter(|success| *success)
            .count();
        assert_eq!(successes, 2);
        assert_eq!(used(&budget), 0);
    }

    #[test]
    fn writing_within_a_lease_does_not_wait_for_shared_accounting() {
        let budget = MemoryBudget::new(32);
        let mut writer = MemoryWriter::bounded(32, budget.clone()).unwrap();
        let held = budget.0.used.lock().unwrap();
        let (tx, rx) = std::sync::mpsc::channel();
        let worker = std::thread::spawn(move || {
            writer.write_all(b"test").unwrap();
            tx.send(writer.into_bytes()).unwrap();
        });
        let completed = rx.recv_timeout(std::time::Duration::from_secs(1));
        drop(held);
        worker.join().unwrap();
        let bytes = completed.unwrap();
        assert_eq!(bytes.as_ref(), b"test");
        assert_eq!(used(&budget), 32);
        assert!(MemoryWriter::bounded(1, budget.clone()).is_err());
        drop(bytes);
        assert_eq!(used(&budget), 0);
    }
}
