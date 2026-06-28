//! First-byte demux of packets seen on the relay channel (STUN vs RTP vs RTCP). Pure: the same
//! classification the sans-IO engine and the platform driver use to route an inbound relay message.

/// Classification of a packet seen on the relay channel, by its first byte.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RelayPacketKind {
    Stun,
    Rtcp,
    Rtp,
    Other,
}

/// First-byte demux: top two bits zero means STUN; 0x80/0x81 means RTCP; 0x90 means RTP
/// (WARP); anything else is other.
pub fn classify_relay_packet(data: &[u8]) -> RelayPacketKind {
    if data.len() < 2 {
        return RelayPacketKind::Other;
    }
    let first = data[0];
    if first & 0xc0 != 0 {
        return match first {
            0x80 | 0x81 => RelayPacketKind::Rtcp,
            0x90 => RelayPacketKind::Rtp,
            _ => RelayPacketKind::Other,
        };
    }
    RelayPacketKind::Stun
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn classify_first_byte() {
        assert_eq!(classify_relay_packet(&[0x00, 0x01]), RelayPacketKind::Stun);
        assert_eq!(classify_relay_packet(&[0x00, 0x03]), RelayPacketKind::Stun);
        assert_eq!(classify_relay_packet(&[0x80, 0xc8]), RelayPacketKind::Rtcp);
        assert_eq!(classify_relay_packet(&[0x81, 0xc8]), RelayPacketKind::Rtcp);
        assert_eq!(classify_relay_packet(&[0x90, 0x78]), RelayPacketKind::Rtp);
        assert_eq!(classify_relay_packet(&[0xff, 0xff]), RelayPacketKind::Other);
        assert_eq!(classify_relay_packet(&[0x00]), RelayPacketKind::Other);
    }
}
