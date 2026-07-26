//! Demux of packets seen on the relay channel (STUN vs RTP vs RTCP). Pure: the same
//! classification the sans-IO engine and the platform driver use to route an inbound relay message.

use super::rtcp::is_rtcp_packet;
use super::rtp::is_rtp_version2;

/// Classification of a packet seen on the relay channel, by its first byte.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RelayPacketKind {
    Stun,
    Rtcp,
    Rtp,
    Other,
}

/// A packet after removing the group relay's forwarding metadata, when present.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RelayPacket<'a> {
    pub payload: &'a [u8],
    pub group_forwarded: bool,
    pub forwarding_header_len: usize,
}

/// A `0x09` packet used an unknown or truncated group-forwarding envelope.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
#[error("malformed group relay forwarding envelope")]
pub struct GroupForwardingError;

/// Remove the group relay's capture-pinned forwarding header.
///
/// Ordinary STUN/RTP/RTCP packets pass through unchanged. A packet beginning with
/// `0x09` is never passed through on parse failure: treating its metadata as media
/// could feed unauthenticated garbage into the RTP/SRTP parsers.
pub fn unwrap_group_forwarding_packet(
    data: &[u8],
) -> Result<RelayPacket<'_>, GroupForwardingError> {
    if data.first() != Some(&0x09) {
        return Ok(RelayPacket {
            payload: data,
            group_forwarded: false,
            forwarding_header_len: 0,
        });
    }

    let forwarding_header_len = match data.get(1) {
        Some(2) => 8,
        Some(4) => 12,
        Some(7) => 18,
        _ => return Err(GroupForwardingError),
    };
    let payload = data
        .get(forwarding_header_len..)
        .filter(|payload| payload.len() >= 12 && payload[0] >> 6 == 2)
        .ok_or(GroupForwardingError)?;
    Ok(RelayPacket {
        payload,
        group_forwarded: true,
        forwarding_header_len,
    })
}

/// RTP/RTCP mux follows RFC 5761's non-overlapping payload-type range. Looking only at byte 0 loses
/// feedback packets whose count/FMT changes it and mistakes ordinary X=0 video RTP for RTCP.
pub fn classify_relay_packet(data: &[u8]) -> RelayPacketKind {
    if data.len() < 2 {
        return RelayPacketKind::Other;
    }
    if data[0] & 0xc0 == 0 {
        return RelayPacketKind::Stun;
    }
    if is_rtcp_packet(data) {
        return RelayPacketKind::Rtcp;
    }
    if is_rtp_version2(data) {
        return RelayPacketKind::Rtp;
    }
    RelayPacketKind::Other
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unwraps_capture_pinned_group_forwarding_envelopes() {
        let vectors = [
            (
                "0902336e0100018890e1001300120c96772e6aa9",
                "90e1001300120c96772e6aa9",
                8,
            ),
            (
                "09047eb5410001000000001090e100010013fbf08f1df407",
                "90e100010013fbf08f1df407",
                12,
            ),
            (
                "0907338f2900020a00c801e000000000000090780001000331808cd481d5",
                "90780001000331808cd481d5",
                18,
            ),
        ];

        for (packet, expected, forwarding_header_len) in vectors {
            let packet = hex::decode(packet).unwrap();
            let expected = hex::decode(expected).unwrap();
            let unwrapped = unwrap_group_forwarding_packet(&packet).unwrap();
            assert_eq!(unwrapped.payload, expected);
            assert!(unwrapped.group_forwarded);
            assert_eq!(unwrapped.forwarding_header_len, forwarding_header_len);
            assert_eq!(
                classify_relay_packet(unwrapped.payload),
                RelayPacketKind::Rtp
            );
        }
    }

    #[test]
    fn group_forwarding_passes_plain_packets_and_rejects_malformed_envelopes() {
        let rtp = [0x90, 0xe1, 0, 1, 0, 0, 0, 1, 0, 0, 0, 2];
        assert_eq!(
            unwrap_group_forwarding_packet(&rtp),
            Ok(RelayPacket {
                payload: &rtp,
                group_forwarded: false,
                forwarding_header_len: 0,
            })
        );

        for packet in [
            &[0x09][..],
            &[0x09, 0xff],
            &[0x09, 0x02, 0, 0, 0, 0, 0, 0],
            &[
                0x09, 0x02, 0, 0, 0, 0, 0, 0, 0x10, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
            ],
        ] {
            assert_eq!(
                unwrap_group_forwarding_packet(packet),
                Err(GroupForwardingError)
            );
        }
    }

    #[test]
    fn classify_first_byte() {
        assert_eq!(classify_relay_packet(&[0x00, 0x01]), RelayPacketKind::Stun);
        assert_eq!(classify_relay_packet(&[0x00, 0x03]), RelayPacketKind::Stun);
        assert_eq!(
            classify_relay_packet(&[0x80, 0xc8, 0, 1, 0, 0, 0, 1]),
            RelayPacketKind::Rtcp
        );
        assert_eq!(
            classify_relay_packet(&[0x8f, 0xce, 0, 2, 0, 0, 0, 1, 0, 0, 0, 2]),
            RelayPacketKind::Rtcp
        );
        let mut video = [0u8; 12];
        video[0] = 0x80;
        video[1] = 0x61;
        assert_eq!(classify_relay_packet(&video), RelayPacketKind::Rtp);
        video[1] = 0xe1;
        assert_eq!(classify_relay_packet(&video), RelayPacketKind::Rtp);
        let mut audio = [0u8; 16];
        audio[0] = 0x90;
        audio[1] = 0x78;
        assert_eq!(classify_relay_packet(&audio), RelayPacketKind::Rtp);
        assert_eq!(classify_relay_packet(&[0xff, 0xff]), RelayPacketKind::Other);
        assert_eq!(classify_relay_packet(&[0x00]), RelayPacketKind::Other);
    }
}
