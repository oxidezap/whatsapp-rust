use std::io::{self, Read};

// A clean EOF ends the stream; bytes from a truncated final frame must not
// disappear from an otherwise successful vector capture.
pub(super) fn read_pcm_frame(input: &mut impl Read, frame: &mut [u8]) -> io::Result<bool> {
    let mut filled = 0;
    while filled < frame.len() {
        match input.read(&mut frame[filled..]) {
            Ok(0) if filled == 0 => return Ok(false),
            Ok(0) => {
                return Err(io::Error::new(
                    io::ErrorKind::UnexpectedEof,
                    "incomplete PCM frame",
                ));
            }
            Ok(count) => filled += count,
            Err(error) if error.kind() == io::ErrorKind::Interrupted => continue,
            Err(error) => return Err(error),
        }
    }
    Ok(true)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pcm_frames_distinguish_clean_eof_from_a_partial_frame() {
        let mut frame = [0; 4];
        let mut complete = &[1, 2, 3, 4][..];
        assert!(read_pcm_frame(&mut complete, &mut frame).unwrap());
        assert_eq!(frame, [1, 2, 3, 4]);
        assert!(!read_pcm_frame(&mut complete, &mut frame).unwrap());

        let mut truncated = &[1, 2, 3][..];
        assert_eq!(
            read_pcm_frame(&mut truncated, &mut frame)
                .unwrap_err()
                .kind(),
            io::ErrorKind::UnexpectedEof
        );
    }
}
