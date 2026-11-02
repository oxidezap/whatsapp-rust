use super::Instant;
use std::sync::Mutex;
use std::time::Duration;

/// Shared by runtime and storage adapters, including blocking storage workers.
/// Wall time must stay in the range accepted by [`ClockReading::new`].
/// Monotonic readings must not go backwards. Reads must not re-enter the
/// owning client or storage adapter.
pub trait Clock: Send + Sync + 'static {
    fn wall_time_millis(&self) -> i64;
    fn monotonic_time(&self) -> Instant;

    /// Override for an independently driven monotonic clock. Otherwise the
    /// clock must advance with the executor that supplies its timers.
    #[cfg(feature = "instance-clock")]
    fn sleep(
        self: std::sync::Arc<Self>,
        _duration: Duration,
    ) -> Option<crate::runtime::BoxFuture<'static, ()>> {
        None
    }

    fn reading(&self) -> Result<ClockReading, ClockError> {
        ClockReading::new(self.wall_time_millis(), self.monotonic_time())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ClockReading {
    wall_millis: i64,
    monotonic: Instant,
}

impl ClockReading {
    pub fn new(wall_millis: i64, monotonic: Instant) -> Result<Self, ClockError> {
        chrono::DateTime::from_timestamp_millis(wall_millis).ok_or(ClockError::WallOutOfRange)?;
        Ok(Self {
            wall_millis,
            monotonic,
        })
    }

    pub fn wall_time_utc(self) -> chrono::DateTime<chrono::Utc> {
        chrono::DateTime::from_timestamp_millis(self.wall_millis).expect("validated clock reading")
    }

    pub fn wall_time_millis(self) -> i64 {
        self.wall_millis
    }
    pub fn wall_time_secs(self) -> i64 {
        self.wall_millis / 1000
    }
    pub fn wall_time_secs_u64(self) -> u64 {
        self.wall_time_secs().max(0) as u64
    }
    pub fn monotonic_time(self) -> Instant {
        self.monotonic
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
#[non_exhaustive]
pub enum ClockError {
    #[error("wall time is outside the supported calendar range")]
    WallOutOfRange,
    #[error("wall time advance overflowed")]
    WallOverflow,
    #[error("monotonic time advance overflowed")]
    MonotonicOverflow,
}

#[derive(Debug, Clone, Copy, Default)]
pub struct GlobalClock;

impl Clock for GlobalClock {
    fn wall_time_millis(&self) -> i64 {
        super::now_millis()
    }
    fn monotonic_time(&self) -> Instant {
        Instant::now()
    }
}

pub struct VirtualClock {
    state: Mutex<VirtualTime>,
    #[cfg(feature = "instance-clock")]
    changed: event_listener::Event,
}

struct VirtualTime {
    reading: ClockReading,
    fractional_nanos: u32,
}

impl VirtualClock {
    pub fn new(wall_millis: i64, monotonic: Instant) -> Result<Self, ClockError> {
        Ok(Self {
            state: Mutex::new(VirtualTime {
                reading: ClockReading::new(wall_millis, monotonic)?,
                fractional_nanos: 0,
            }),
            #[cfg(feature = "instance-clock")]
            changed: event_listener::Event::new(),
        })
    }

    pub fn advance(&self, duration: Duration) -> Result<ClockReading, ClockError> {
        let mut state = self
            .state
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let monotonic = state
            .reading
            .monotonic
            .checked_add(duration)
            .ok_or(ClockError::MonotonicOverflow)?;
        let nanos = duration.as_nanos() + u128::from(state.fractional_nanos);
        let millis = i64::try_from(nanos / 1_000_000).map_err(|_| ClockError::WallOverflow)?;
        let wall = state
            .reading
            .wall_millis
            .checked_add(millis)
            .ok_or(ClockError::WallOverflow)?;
        let reading = ClockReading::new(wall, monotonic)?;
        state.reading = reading;
        state.fractional_nanos = (nanos % 1_000_000) as u32;
        drop(state);
        #[cfg(feature = "instance-clock")]
        if !duration.is_zero() {
            self.changed.notify(usize::MAX);
        }
        Ok(reading)
    }

    pub fn set_wall_time_millis(&self, wall_millis: i64) -> Result<ClockReading, ClockError> {
        let mut state = self
            .state
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        state.reading = ClockReading::new(wall_millis, state.reading.monotonic)?;
        state.fractional_nanos = 0;
        Ok(state.reading)
    }
}

impl Clock for VirtualClock {
    #[cfg(feature = "instance-clock")]
    fn sleep(
        self: std::sync::Arc<Self>,
        duration: Duration,
    ) -> Option<crate::runtime::BoxFuture<'static, ()>> {
        let deadline = self.monotonic_time().checked_add(duration);
        Some(Box::pin(async move {
            let Some(deadline) = deadline else {
                return std::future::pending().await;
            };
            loop {
                let changed = self.changed.listen();
                if self.monotonic_time() >= deadline {
                    return;
                }
                changed.await;
            }
        }))
    }

    fn wall_time_millis(&self) -> i64 {
        self.state
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .reading
            .wall_millis
    }

    fn monotonic_time(&self) -> Instant {
        self.state
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .reading
            .monotonic
    }

    fn reading(&self) -> Result<ClockReading, ClockError> {
        Ok(self
            .state
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .reading)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn independent_clocks_do_not_install_process_providers() {
        let a = VirtualClock::new(1000, Instant::ZERO).unwrap();
        let b = VirtualClock::new(9000, Instant::ZERO).unwrap();
        a.advance(Duration::from_secs(2)).unwrap();
        assert_eq!(a.wall_time_millis(), 3000);
        assert_eq!(b.wall_time_millis(), 9000);
        assert_eq!(b.monotonic_time(), Instant::ZERO);
    }

    #[test]
    fn submillisecond_advances_accumulate_without_drift() {
        let clock = VirtualClock::new(0, Instant::ZERO).unwrap();
        for _ in 0..1000 {
            clock.advance(Duration::from_nanos(1000)).unwrap();
        }
        assert_eq!(clock.wall_time_millis(), 1);
        assert_eq!(
            clock.monotonic_time(),
            Instant::ZERO + Duration::from_millis(1)
        );
    }

    #[test]
    fn wall_clock_jumps_do_not_change_monotonic_deadlines() {
        let clock = VirtualClock::new(1000, Instant::ZERO).unwrap();
        clock.advance(Duration::from_secs(2)).unwrap();
        let before = clock.monotonic_time();
        clock.set_wall_time_millis(-1000).unwrap();
        assert_eq!(clock.monotonic_time(), before);
        assert_eq!(clock.reading().unwrap().wall_time_secs_u64(), 0);
    }

    #[test]
    fn failed_advance_changes_neither_clock() {
        let clock = VirtualClock::new(0, Instant::ZERO + Duration::from_nanos(u64::MAX)).unwrap();
        let before = clock.reading().unwrap();
        assert_eq!(
            clock.advance(Duration::from_nanos(1)),
            Err(ClockError::MonotonicOverflow)
        );
        assert_eq!(clock.reading().unwrap(), before);
        assert_eq!(
            clock.set_wall_time_millis(i64::MAX),
            Err(ClockError::WallOutOfRange)
        );
        assert_eq!(clock.reading().unwrap(), before);
    }
}
