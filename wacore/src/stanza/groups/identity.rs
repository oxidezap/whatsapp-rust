//! The group parser's typed JID accessors, qualified against WAJids and
//! WAParsableWapNode in WA 2.3000.1047483476. Validation precedes the core JID
//! parser so permissive aliases/device forms cannot widen an upstream accessor.
//!
//! WAJids: bundle 09d91fdc4089c50ab1d35dd6c796ee82ef4c3296710316d20383c85738b871b8,
//! bytes 2184438..2196682, module SHA-256
//! 8b8655512bfc775d31954e5101970d1774e2c0800e4608698be13105cc23fb0c.
//! Acorn-indexed regex byte spans relative to that module: group `_` 172..236,
//! interop `v` 349..402, msgr `R` 454..481 and `L` 484..516,
//! phone `N` 735..797, lid `A` 817..843, bot `K` 1132..1161.
//! Validation is limited to the core's known, representable server spellings.
//! WAParsableWapNode: same bundle, 2232374..2241375.
//! WAParsableXmlNode attrInt: bundle 082e0ef2faaa6247e6e17a225bb1685b7e583e946f7b0dc17ee58c431c1a07a8,
//! bytes 1112662..1117469, module SHA-256 5d238b9f2e2ecde2141c039a19811f665104b3ab03a149578004a402f492681a.
use wacore_binary::Jid;

fn decimal(s: &str, min: usize, max: usize) -> bool {
    (min..=max).contains(&s.len())
        && s.as_bytes()
            .first()
            .is_some_and(|b| (b'1'..=b'9').contains(b))
        && s.bytes().all(|b| b.is_ascii_digit())
}
fn phone_user(user: &str) -> bool {
    if user == "0" {
        return true;
    }
    let (number, agent) = user
        .split_once('.')
        .map_or((user, None), |(n, a)| (n, Some(a)));
    !number.starts_with("10")
        && decimal(number, 5, 20)
        && agent.is_none_or(|a| (1..=2).contains(&a.len()) && a.bytes().all(|b| b.is_ascii_digit()))
}
fn split(input: &str) -> Option<(&str, &str)> {
    let (user, server) = input.split_once('@')?;
    (!server.contains('@')).then_some((user, server))
}
/// Generic user accessor includes PN/LID, Messenger, interop and bot identities.
/// Device and hosted-device JIDs are not user JIDs in this official classifier.
pub(super) fn parse_user(input: &str) -> Option<Jid> {
    let (user, server) = split(input)?;
    let valid = match server {
        "s.whatsapp.net" => phone_user(user),
        "lid" => decimal(user, 1, 15),
        "msgr" => decimal(user.strip_suffix(":0").unwrap_or(user), 1, 20),
        "bot" => decimal(user.strip_suffix(":0").unwrap_or(user), 1, 20),
        "interop" => {
            let user = user.strip_suffix(":0").unwrap_or(user);
            user.split_once('-').is_some_and(|(integrator, number)| {
                decimal(integrator, 1, 3) && decimal(number, 1, 15)
            })
        }
        _ => false,
    };
    valid.then(|| input.parse().ok()).flatten()
}
pub(super) fn parse_phone(input: &str) -> Option<Jid> {
    let (user, server) = split(input)?;
    (server == "s.whatsapp.net" && phone_user(user))
        .then(|| input.parse().ok())
        .flatten()
}
pub(super) fn parse_lid(input: &str) -> Option<Jid> {
    let (user, server) = split(input)?;
    (server == "lid" && decimal(user, 1, 15))
        .then(|| input.parse().ok())
        .flatten()
}
pub(super) fn parse_group(input: &str) -> Option<Jid> {
    let (user, server) = split(input)?;
    let valid = decimal(user, 1, 20)
        || user.split_once('-').is_some_and(|(creator, time)| {
            !creator.starts_with("10") && decimal(creator, 5, 20) && decimal(time, 10, 10)
        });
    (server == "g.us" && valid)
        .then(|| input.parse().ok())
        .flatten()
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum IntegerReadError {
    NotInteger,
    OutsideExactRange,
}
/// JS parseInt(value,10), bounded to the exact safe-integer range. The caller
/// must report unsupported numeric range explicitly instead of rounding it.
pub(super) fn parse_js_integer(input: &str) -> Result<i64, IntegerReadError> {
    let input = input.trim_start_matches(|c: char| {
        matches!(
            c,
            '\t' | '\n'
                | '\u{000b}'
                | '\u{000c}'
                | '\r'
                | ' '
                | '\u{00a0}'
                | '\u{1680}'
                | '\u{2000}'
                ..='\u{200a}'
                    | '\u{2028}'
                    | '\u{2029}'
                    | '\u{202f}'
                    | '\u{205f}'
                    | '\u{3000}'
                    | '\u{feff}'
        )
    });
    let (negative, digits) = input.strip_prefix('-').map_or_else(
        || (false, input.strip_prefix('+').unwrap_or(input)),
        |s| (true, s),
    );
    let mut value = 0i64;
    let mut count = 0;
    for b in digits.bytes().take_while(u8::is_ascii_digit) {
        value = value
            .checked_mul(10)
            .and_then(|v| v.checked_add(i64::from(b - b'0')))
            .filter(|v| *v <= 9_007_199_254_740_991)
            .ok_or(IntegerReadError::OutsideExactRange)?;
        count += 1;
    }
    if count == 0 {
        return Err(IntegerReadError::NotInteger);
    }
    Ok(if negative { -value } else { value })
}

/// WAParsableWapNode.attrTime delegates to WATimeUtils.castToUnixTime, whose
/// ToInt32 wrap is followed by exclusion of the reserved minimum-i32 sentinel.
/// WATimeUtils in the same bundle: bytes 2198607..2202055, function k().
pub(super) fn parse_js_time(input: &str) -> Result<i64, IntegerReadError> {
    parse_js_integer(input).map(|value| i64::from(value as i32).max(-2_147_483_647))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn official_user_classes_are_distinct_from_devices_and_aliases() {
        for jid in [
            "200002@s.whatsapp.net",
            "0@s.whatsapp.net",
            "200002.2@s.whatsapp.net",
            "100001@lid",
            "200002@msgr",
            "200002:0@msgr",
            "1-200002@interop",
            "200002@bot",
            "200002:0@bot",
        ] {
            assert!(parse_user(jid).is_some(), "{jid}");
        }
        for jid in [
            "100002@s.whatsapp.net",
            "2@s.whatsapp.net",
            "0200002@s.whatsapp.net",
            "200002:2@s.whatsapp.net",
            "100001:2@lid",
            "200002:99@hosted",
            "100001:99@hosted.lid",
            "200002@c.us",
            "x@lid",
            "0@lid",
            "1234567890123456@lid",
            "1-0@interop",
        ] {
            assert!(parse_user(jid).is_none(), "{jid}");
        }
        assert!(parse_phone("200002.12@s.whatsapp.net").is_some());
        assert!(parse_phone("200002.123@s.whatsapp.net").is_none());
        assert!(parse_phone("100001@lid").is_none());
        assert!(parse_lid("100001@lid").is_some());
        assert!(parse_lid("100001:0@lid").is_none());
    }
    #[test]
    fn group_class_matches_modern_and_creator_timestamp_forms() {
        for jid in [
            "1@g.us",
            "12345678901234567890@g.us",
            "200002-1234567890@g.us",
        ] {
            assert!(parse_group(jid).is_some(), "{jid}");
        }
        for jid in [
            "0@g.us",
            "100002-1234567890@g.us",
            "200002-123@g.us",
            "123456789012345678901@g.us",
            "123456@lid",
        ] {
            assert!(parse_group(jid).is_none(), "{jid}");
        }
    }
    #[test]
    fn decimal_reads_have_js_prefix_and_whitespace_semantics() {
        for (input, expected) in [
            ("  +12seconds", 12),
            ("-2.9", -2),
            ("0x20", 0),
            ("\u{feff}3", 3),
            ("0012", 12),
        ] {
            assert_eq!(parse_js_integer(input), Ok(expected));
        }
        assert_eq!(
            parse_js_integer("future"),
            Err(IntegerReadError::NotInteger)
        );
        assert_eq!(
            parse_js_integer("\u{0085}3"),
            Err(IntegerReadError::NotInteger)
        );
        assert_eq!(
            parse_js_integer("9007199254740992"),
            Err(IntegerReadError::OutsideExactRange)
        );
    }
    #[test]
    fn unix_times_wrap_and_clamp_the_reserved_i32_sentinel() {
        for (input, expected) in [
            ("4294967297", 1),
            ("2147483648", -2_147_483_647),
            ("4294967296", 0),
            ("-2147483649", 2_147_483_647),
        ] {
            assert_eq!(parse_js_time(input), Ok(expected));
        }
    }
}
