use core::{fmt, num::NonZeroU64, time::Duration};
use serde::{Deserialize, Deserializer, Serialize, Serializer};

/// A UTC instant with millisecond precision.
///
/// JSON uses signed Unix milliseconds. Fields whose protocol specifies seconds
/// use an explicit seconds serializer instead. No timezone or calendar state is
/// stored. Validation preserves the SDK's existing timestamp range and
/// invalid-input handling.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[repr(transparent)]
pub struct Timestamp(NonZeroU64);

impl Timestamp {
    const SIGN: u64 = 1 << 63;
    /// Earliest accepted Unix millisecond timestamp.
    pub const MIN_MILLIS: i64 = -8_334_601_228_800_000;
    /// Latest accepted Unix millisecond timestamp.
    pub const MAX_MILLIS: i64 = 8_210_266_876_799_999;
    /// The Unix epoch is an ordinary representable instant, not an unset value.
    pub const UNIX_EPOCH: Self =
        Self(NonZeroU64::new(Self::SIGN).expect("epoch encoding is nonzero"));

    /// Validate Unix milliseconds against the supported timestamp range.
    pub const fn from_millis(millis: i64) -> Option<Self> {
        if millis < Self::MIN_MILLIS || millis > Self::MAX_MILLIS {
            return None;
        }
        // Flipping the sign bit preserves signed ordering in an unsigned word.
        // Only i64::MIN encodes to zero, and the range above excludes it, so
        // Option<Timestamp> can use that niche without treating epoch as unset.
        match NonZeroU64::new((millis as u64) ^ Self::SIGN) {
            Some(value) => Some(Self(value)),
            None => None,
        }
    }

    /// Validate Unix seconds without overflowing the millisecond conversion.
    pub const fn from_secs(seconds: i64) -> Option<Self> {
        match seconds.checked_mul(1000) {
            Some(millis) => Self::from_millis(millis),
            None => None,
        }
    }

    /// Signed milliseconds since the Unix epoch.
    pub const fn as_millis(self) -> i64 {
        (self.0.get() ^ Self::SIGN) as i64
    }

    /// Whole Unix seconds, rounding toward the earlier instant before epoch.
    pub const fn as_secs(self) -> i64 {
        self.as_millis().div_euclid(1000)
    }

    /// Add an exactly representable duration. Returns `None` for sub-millisecond
    /// precision, arithmetic overflow or a result outside the timestamp range.
    pub fn checked_add(self, duration: Duration) -> Option<Self> {
        if !duration.subsec_nanos().is_multiple_of(1_000_000) {
            return None;
        }
        let millis = i64::try_from(duration.as_millis()).ok()?;
        Self::from_millis(self.as_millis().checked_add(millis)?)
    }

    /// Subtract an exactly representable duration, with the same bounds as
    /// [`Self::checked_add`].
    pub fn checked_sub(self, duration: Duration) -> Option<Self> {
        if !duration.subsec_nanos().is_multiple_of(1_000_000) {
            return None;
        }
        let millis = i64::try_from(duration.as_millis()).ok()?;
        Self::from_millis(self.as_millis().checked_sub(millis)?)
    }

    /// Elapsed time since `earlier`, or `None` if the wall clock went backwards.
    pub fn duration_since(self, earlier: Self) -> Option<Duration> {
        // The validated range's entire width fits i64, including MIN to MAX.
        let millis = u64::try_from(self.as_millis() - earlier.as_millis()).ok()?;
        Some(Duration::from_millis(millis))
    }

    /// A UTC clock display, `HH:MM:SS`, without an allocation.
    pub fn display_hms(self) -> ClockDisplay {
        ClockDisplay {
            timestamp: self,
            millis: false,
        }
    }

    /// A UTC clock display, `HH:MM:SS.mmm`, without an allocation.
    pub fn display_hms_millis(self) -> ClockDisplay {
        ClockDisplay {
            timestamp: self,
            millis: true,
        }
    }
}

impl Default for Timestamp {
    fn default() -> Self {
        Self::UNIX_EPOCH
    }
}

impl fmt::Debug for Timestamp {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Timestamp({}ms)", self.as_millis())
    }
}

impl fmt::Display for Timestamp {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.as_millis().fmt(f)
    }
}

impl Serialize for Timestamp {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_i64(self.as_millis())
    }
}

impl<'de> Deserialize<'de> for Timestamp {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let millis = i64::deserialize(deserializer)?;
        Self::from_millis(millis).ok_or_else(|| serde::de::Error::custom("timestamp out of range"))
    }
}

/// Stack-only formatting view returned by [`Timestamp::display_hms`].
#[derive(Debug, Clone, Copy)]
pub struct ClockDisplay {
    timestamp: Timestamp,
    millis: bool,
}

impl fmt::Display for ClockDisplay {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let millis = self.timestamp.as_millis().rem_euclid(86_400_000);
        let seconds = millis / 1000;
        write!(
            f,
            "{:02}:{:02}:{:02}",
            seconds / 3600,
            seconds / 60 % 60,
            seconds % 60
        )?;
        if self.millis {
            write!(f, ".{:03}", millis % 1000)?;
        }
        Ok(())
    }
}

/// Serde adapter for protocol fields measured in Unix seconds.
pub mod serde_seconds {
    use super::*;
    pub fn serialize<S: Serializer>(value: &Timestamp, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_i64(value.as_secs())
    }
    pub fn deserialize<'de, D: Deserializer<'de>>(deserializer: D) -> Result<Timestamp, D::Error> {
        let seconds = i64::deserialize(deserializer)?;
        Timestamp::from_secs(seconds)
            .ok_or_else(|| serde::de::Error::custom("timestamp out of range"))
    }
}

/// Serde adapter for optional protocol timestamps measured in Unix seconds.
pub mod serde_seconds_option {
    use super::*;
    pub fn serialize<S: Serializer>(
        value: &Option<Timestamp>,
        serializer: S,
    ) -> Result<S::Ok, S::Error> {
        value.map(Timestamp::as_secs).serialize(serializer)
    }
    pub fn deserialize<'de, D: Deserializer<'de>>(
        deserializer: D,
    ) -> Result<Option<Timestamp>, D::Error> {
        Option::<i64>::deserialize(deserializer)?
            .map(|seconds| {
                Timestamp::from_secs(seconds)
                    .ok_or_else(|| serde::de::Error::custom("timestamp out of range"))
            })
            .transpose()
    }
}

pub(crate) fn serialize_duration<S: Serializer>(
    value: &Duration,
    serializer: S,
) -> Result<S::Ok, S::Error> {
    (value.as_secs(), value.subsec_nanos()).serialize(serializer)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn compact_values_preserve_order_epoch_and_domain_edges() {
        assert_eq!(size_of::<Timestamp>(), 8);
        assert_eq!(size_of::<Option<Timestamp>>(), 8);
        let values = [
            Timestamp::MIN_MILLIS,
            -1001,
            -1000,
            -1,
            0,
            1,
            999,
            1000,
            Timestamp::MAX_MILLIS,
        ];
        let stamps = values.map(|millis| Timestamp::from_millis(millis).unwrap());
        for (stamp, millis) in stamps.iter().zip(values) {
            assert_eq!(stamp.as_millis(), millis);
            assert_eq!(stamp.as_secs(), millis.div_euclid(1000));
        }
        assert!(stamps.windows(2).all(|pair| pair[0] < pair[1]));
        assert_eq!(Timestamp::from_millis(0), Some(Timestamp::UNIX_EPOCH));
        for invalid in [
            i64::MIN,
            Timestamp::MIN_MILLIS - 1,
            Timestamp::MAX_MILLIS + 1,
            i64::MAX,
        ] {
            assert_eq!(Timestamp::from_millis(invalid), None);
        }
        assert_eq!(Timestamp::from_secs(i64::MAX), None);
        assert_eq!(Timestamp::from_secs(i64::MIN), None);
    }

    #[test]
    fn checked_durations_preserve_precision_and_handle_backwards_time() {
        let epoch = Timestamp::UNIX_EPOCH;
        let before = epoch.checked_sub(Duration::from_millis(1)).unwrap();
        assert_eq!(before.as_millis(), -1);
        assert_eq!(before.as_secs(), -1);
        assert_eq!(before.duration_since(epoch), None);
        assert_eq!(epoch.duration_since(before), Some(Duration::from_millis(1)));
        assert_eq!(epoch.checked_add(Duration::from_nanos(1)), None);
        assert_eq!(epoch.checked_sub(Duration::from_nanos(1)), None);
        assert_eq!(epoch.checked_add(Duration::MAX), None);
        assert_eq!(
            Timestamp::from_millis(Timestamp::MAX_MILLIS)
                .unwrap()
                .checked_add(Duration::from_millis(1)),
            None
        );
        assert_eq!(
            Timestamp::from_millis(Timestamp::MIN_MILLIS)
                .unwrap()
                .checked_sub(Duration::from_millis(1)),
            None
        );
        let min = Timestamp::from_millis(Timestamp::MIN_MILLIS).unwrap();
        let max = Timestamp::from_millis(Timestamp::MAX_MILLIS).unwrap();
        let width = i128::from(Timestamp::MAX_MILLIS) - i128::from(Timestamp::MIN_MILLIS);
        assert!(width <= i128::from(i64::MAX));
        assert_eq!(
            max.duration_since(min),
            Some(Duration::from_millis(u64::try_from(width).unwrap()))
        );
        assert_eq!(min.duration_since(max), None);
    }

    #[test]
    fn timestamp_json_is_signed_milliseconds_and_validates_input() {
        for millis in [
            Timestamp::MIN_MILLIS,
            -1001,
            -1,
            0,
            1,
            1_700_000_000_123,
            Timestamp::MAX_MILLIS,
        ] {
            let stamp = Timestamp::from_millis(millis).unwrap();
            assert_eq!(serde_json::to_string(&stamp).unwrap(), millis.to_string());
            assert_eq!(
                serde_json::from_str::<Timestamp>(&millis.to_string()).unwrap(),
                stamp
            );
        }
        assert!(serde_json::from_str::<Timestamp>(&i64::MAX.to_string()).is_err());
        assert!(serde_json::from_str::<Timestamp>("\"1970-01-01T00:00:00Z\"").is_err());
        assert_eq!(
            serde_json::to_string(&Some(Timestamp::UNIX_EPOCH)).unwrap(),
            "0"
        );
        assert_eq!(
            serde_json::to_string(&Option::<Timestamp>::None).unwrap(),
            "null"
        );
        assert_eq!(
            serde_json::from_str::<Option<Timestamp>>("0").unwrap(),
            Some(Timestamp::UNIX_EPOCH)
        );
        assert_eq!(
            serde_json::from_str::<Option<Timestamp>>("null").unwrap(),
            None
        );
    }

    #[test]
    fn displays_use_utc_modulo_days_including_before_epoch() {
        let before = Timestamp::from_millis(-1).unwrap();
        assert_eq!(before.display_hms().to_string(), "23:59:59");
        assert_eq!(before.display_hms_millis().to_string(), "23:59:59.999");
        assert_eq!(Timestamp::UNIX_EPOCH.display_hms().to_string(), "00:00:00");
        assert_eq!(
            Timestamp::from_millis(86_400_001)
                .unwrap()
                .display_hms_millis()
                .to_string(),
            "00:00:00.001"
        );
    }
}
