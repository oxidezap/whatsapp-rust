#![cfg(feature = "voip-mlow")]

use std::sync::Mutex;

use log::{Level, LevelFilter, Log, Metadata, Record};
use wacore::voip::mlow::{MlowDecoder, MlowEncoder};

struct DecoderLogs(Mutex<Vec<(Level, String)>>);

impl Log for DecoderLogs {
    fn enabled(&self, metadata: &Metadata<'_>) -> bool {
        metadata.target() == "wacore::voip::mlow::decoder" && metadata.level() <= log::max_level()
    }

    fn log(&self, record: &Record<'_>) {
        if self.enabled(record.metadata()) {
            self.0
                .lock()
                .unwrap()
                .push((record.level(), record.args().to_string()));
        }
    }

    fn flush(&self) {}
}

#[test]
fn mlow_success_logs_require_trace_without_changing_decode_output() {
    static LOGS: DecoderLogs = DecoderLogs(Mutex::new(Vec::new()));
    log::set_logger(&LOGS).unwrap();

    let pcm: Vec<f32> = (0..960)
        .map(|i| (i as f32 * std::f32::consts::TAU * 440.0 / 16000.0).sin() * 0.25)
        .collect();
    let mut encoder = MlowEncoder::new();
    let packets = [
        encoder.encode(&pcm).unwrap(),
        vec![0x90],
        encoder.encode(&pcm).unwrap(),
    ];
    let mut outputs = Vec::new();
    for level in [LevelFilter::Debug, LevelFilter::Trace] {
        log::set_max_level(level);
        LOGS.0.lock().unwrap().clear();
        let mut decoder = MlowDecoder::new();
        let decoded: Vec<_> = packets
            .iter()
            .map(|packet| {
                let samples = decoder.decode(packet);
                (samples, decoder.take_frame_report())
            })
            .collect();
        assert_eq!(decoded[0].1.decoded, 3);
        assert_eq!(decoded[1].1.inactive_or_sid, 1);
        assert_eq!(decoded[2].1.decoded, 3);
        let logs = LOGS.0.lock().unwrap();
        if level == LevelFilter::Debug {
            assert!(logs.is_empty(), "success logs at debug: {logs:?}");
        } else {
            assert_eq!(logs.len(), 3, "{logs:?}");
            assert!(logs.iter().all(|(level, _)| *level == Level::Trace));
            assert!(logs[0].1.starts_with("mlow: active frame decoded"));
            assert!(logs[1].1.starts_with("mlow: SID TOC"));
            assert!(logs[2].1.starts_with("mlow: active frame decoded"));
        }
        outputs.push(decoded);
    }
    assert_eq!(outputs[0], outputs[1]);

    log::set_max_level(LevelFilter::Debug);
    LOGS.0.lock().unwrap().clear();
    let mut decoder = MlowDecoder::new();
    for packet in [&[0xc0][..], &[0x70][..]] {
        decoder.decode(packet);
        assert_eq!(decoder.take_frame_report().off_point, 1);
    }
    let logs = LOGS.0.lock().unwrap();
    assert_eq!(logs.len(), 2, "{logs:?}");
    assert_eq!(logs[0].0, Level::Debug);
    assert!(logs[0].1.starts_with("mlow: standard-opus TOC"));
    assert_eq!(logs[1].0, Level::Warn);
    assert!(
        logs[1]
            .1
            .starts_with("mlow: dropping out-of-operating-point")
    );
}
