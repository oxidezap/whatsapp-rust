/// Validate the UTC RFC3339 acquisition strings accepted by the capture schema.
/// Leap seconds are syntactically valid, as in the previous parser; deciding
/// whether a leap second occurred on a date requires an external time database.
pub(crate) fn valid_acquisition_time(value: &str) -> bool {
    let bytes = value.as_bytes();
    if bytes.len() < 20
        || bytes[4] != b'-'
        || bytes[7] != b'-'
        || !matches!(bytes[10], b'T' | b't' | b' ')
        || bytes[13] != b':'
        || bytes[16] != b':'
        || bytes.last() != Some(&b'Z')
    {
        return false;
    }
    fn decimal(bytes: &[u8]) -> Option<u16> {
        bytes.iter().try_fold(0_u16, |number, &byte| {
            byte.is_ascii_digit()
                .then(|| number * 10 + u16::from(byte - b'0'))
        })
    }
    let (Some(year), Some(month), Some(day), Some(hour), Some(minute), Some(second)) = (
        decimal(&bytes[..4]),
        decimal(&bytes[5..7]),
        decimal(&bytes[8..10]),
        decimal(&bytes[11..13]),
        decimal(&bytes[14..16]),
        decimal(&bytes[17..19]),
    ) else {
        return false;
    };
    let max_day = match month {
        1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
        4 | 6 | 9 | 11 => 30,
        2 if year.is_multiple_of(400) || year.is_multiple_of(4) && !year.is_multiple_of(100) => 29,
        2 => 28,
        _ => return false,
    };
    if day == 0 || day > max_day || hour > 23 || minute > 59 || second > 60 {
        return false;
    }
    bytes.len() == 20
        || bytes.len() > 21
            && bytes[19] == b'.'
            && bytes[20..bytes.len() - 1].iter().all(u8::is_ascii_digit)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn utc_capture_timestamp_preserves_calendar_and_fraction_boundaries() {
        for valid in [
            "2024-02-29T23:59:60Z",
            "2000-02-29T00:00:00Z",
            "0000-02-29T00:00:00Z",
            "2024-02-29t12:30:00Z",
            "2024-02-29 12:30:00Z",
            "2024-02-29T12:30:00.123456789012Z",
            "9999-12-31T23:59:59Z",
        ] {
            assert!(valid_acquisition_time(valid), "{valid}");
        }
        for invalid in [
            "2023-02-29T23:59:00Z",
            "1900-02-29T00:00:00Z",
            "2024-04-31T00:00:00Z",
            "2024-00-01T00:00:00Z",
            "2024-01-00T00:00:00Z",
            "2024-01-01T24:00:00Z",
            "2024-01-01T00:60:00Z",
            "2024-01-01T00:00:61Z",
            "2024-01-01T00:00:00.Z",
            "2024-01-01T00:00:00z",
            "2024-01-01T00:00:00+00:00",
            "2024-01-01T00:00:00Z ",
            "2024-01-01T00:00:00.éZ",
            "2024-01-01",
            "",
        ] {
            assert!(!valid_acquisition_time(invalid), "{invalid}");
        }
    }
}
