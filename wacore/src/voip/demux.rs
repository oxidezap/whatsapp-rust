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
