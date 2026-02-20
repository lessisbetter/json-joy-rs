use json_joy_json_pack::ws::{WsFrame, WsFrameDecoder, WsFrameEncoder, WsFrameOpcode};

fn read_frame(decoder: &mut WsFrameDecoder) -> WsFrame {
    decoder
        .read_frame_header()
        .unwrap_or_else(|e| panic!("decode failed: {e}"))
        .unwrap_or_else(|| panic!("expected frame"))
}

#[test]
fn ws_control_frame_encode_decode_matrix() {
    let mut encoder = WsFrameEncoder::new();
    let mut decoder = WsFrameDecoder::new();

    decoder.push(encoder.encode_ping(None));
    match read_frame(&mut decoder) {
        WsFrame::Ping(frame) => {
            assert!(frame.header.fin);
            assert_eq!(frame.header.opcode, WsFrameOpcode::Ping as u8);
            assert_eq!(frame.header.length, 0);
            assert!(frame.header.mask.is_none());
            assert!(frame.data.is_empty());
        }
        other => panic!("expected ping, got {other:?}"),
    }

    decoder.push(encoder.encode_ping(Some(&[1, 2, 3, 4])));
    match read_frame(&mut decoder) {
        WsFrame::Ping(frame) => {
            assert!(frame.header.fin);
            assert_eq!(frame.header.opcode, WsFrameOpcode::Ping as u8);
            assert_eq!(frame.header.length, 4);
            assert!(frame.header.mask.is_none());
            assert_eq!(frame.data, vec![1, 2, 3, 4]);
        }
        other => panic!("expected ping, got {other:?}"),
    }

    decoder.push(encoder.encode_pong(None));
    match read_frame(&mut decoder) {
        WsFrame::Pong(frame) => {
            assert!(frame.header.fin);
            assert_eq!(frame.header.opcode, WsFrameOpcode::Pong as u8);
            assert_eq!(frame.header.length, 0);
            assert!(frame.header.mask.is_none());
            assert!(frame.data.is_empty());
        }
        other => panic!("expected pong, got {other:?}"),
    }

    decoder.push(encoder.encode_pong(Some(&[1, 2, 3, 4])));
    match read_frame(&mut decoder) {
        WsFrame::Pong(frame) => {
            assert!(frame.header.fin);
            assert_eq!(frame.header.opcode, WsFrameOpcode::Pong as u8);
            assert_eq!(frame.header.length, 4);
            assert!(frame.header.mask.is_none());
            assert_eq!(frame.data, vec![1, 2, 3, 4]);
        }
        other => panic!("expected pong, got {other:?}"),
    }

    decoder.push(encoder.encode_close("", 0));
    match read_frame(&mut decoder) {
        WsFrame::Close(frame) => {
            assert!(frame.header.fin);
            assert_eq!(frame.header.opcode, WsFrameOpcode::Close as u8);
            assert_eq!(frame.header.length, 0);
            assert!(frame.header.mask.is_none());
            assert_eq!(frame.code, 0);
            assert!(frame.reason.is_empty());
        }
        other => panic!("expected close, got {other:?}"),
    }

    decoder.push(encoder.encode_close("gg wp", 123));
    match read_frame(&mut decoder) {
        WsFrame::Close(mut frame) => {
            decoder
                .read_close_frame_data(&mut frame)
                .unwrap_or_else(|e| panic!("close decode failed: {e}"));
            assert!(frame.header.fin);
            assert_eq!(frame.header.opcode, WsFrameOpcode::Close as u8);
            assert_eq!(frame.header.length, 7);
            assert!(frame.header.mask.is_none());
            assert_eq!(frame.code, 123);
            assert_eq!(frame.reason, "gg wp");
        }
        other => panic!("expected close, got {other:?}"),
    }
}

#[test]
fn ws_data_frame_header_size_matrix() {
    let sizes = [
        0usize,
        1,
        2,
        125,
        126,
        127,
        128,
        129,
        255,
        1234,
        65_535,
        65_536,
        65_537,
        7_777_777,
        (1usize << 31) - 1,
    ];
    let mut encoder = WsFrameEncoder::new();
    let mut decoder = WsFrameDecoder::new();

    for size in sizes {
        decoder.push(encoder.encode_hdr(true, WsFrameOpcode::Binary, size, 0));
        match read_frame(&mut decoder) {
            WsFrame::Data(frame) => {
                assert!(frame.fin);
                assert_eq!(frame.opcode, WsFrameOpcode::Binary as u8);
                assert_eq!(frame.length, size);
                assert!(frame.mask.is_none());
            }
            other => panic!("expected data frame for size {size}, got {other:?}"),
        }
    }
}

#[test]
fn ws_masked_and_fragmented_data_matrix() {
    let mut encoder = WsFrameEncoder::new();
    let mut decoder = WsFrameDecoder::new();

    let data = [1u8, 2, 3, 4, 5];
    let mask = 123_456_789u32;
    encoder.write_hdr(true, WsFrameOpcode::Binary, data.len(), mask);
    encoder.write_buf_xor(&data, mask);
    decoder.push(encoder.writer.flush());

    match read_frame(&mut decoder) {
        WsFrame::Data(frame) => {
            assert!(frame.fin);
            assert_eq!(frame.opcode, WsFrameOpcode::Binary as u8);
            assert_eq!(frame.length, data.len());
            assert_eq!(frame.mask, Some([7, 91, 205, 21]));
            let decoded = decoder.reader.buf_xor(frame.length, frame.mask.unwrap(), 0);
            assert_eq!(decoded, data);
        }
        other => panic!("expected masked data frame, got {other:?}"),
    }

    let data1 = [1u8, 2, 3];
    let data2 = [4u8, 5];
    let mask1 = 333_444_555u32;
    let mask2 = 123_123_123u32;
    encoder.write_hdr(false, WsFrameOpcode::Binary, data1.len(), mask1);
    encoder.write_buf_xor(&data1, mask1);
    encoder.write_hdr(true, WsFrameOpcode::Continue, data2.len(), mask2);
    encoder.write_buf_xor(&data2, mask2);
    decoder.push(encoder.writer.flush());

    match read_frame(&mut decoder) {
        WsFrame::Data(frame) => {
            assert!(!frame.fin);
            assert_eq!(frame.opcode, WsFrameOpcode::Binary as u8);
            assert_eq!(frame.length, data1.len());
            let decoded = decoder.reader.buf_xor(frame.length, frame.mask.unwrap(), 0);
            assert_eq!(decoded, data1);
        }
        other => panic!("expected fragmented first frame, got {other:?}"),
    }

    match read_frame(&mut decoder) {
        WsFrame::Data(frame) => {
            assert!(frame.fin);
            assert_eq!(frame.opcode, WsFrameOpcode::Continue as u8);
            assert_eq!(frame.length, data2.len());
            let decoded = decoder.reader.buf_xor(frame.length, frame.mask.unwrap(), 0);
            assert_eq!(decoded, data2);
        }
        other => panic!("expected fragmented continuation, got {other:?}"),
    }
}

#[test]
fn ws_decoder_text_payload_matrix() {
    let mut decoder = WsFrameDecoder::new();
    decoder.push(vec![
        129, 136, // header
        136, 35, 93, 205, // mask
        231, 85, 56, 191, 177, 19, 109, 253, // payload
    ]);
    match read_frame(&mut decoder) {
        WsFrame::Data(frame) => {
            assert!(frame.fin);
            assert_eq!(frame.opcode, WsFrameOpcode::Text as u8);
            assert_eq!(frame.length, 8);
            assert_eq!(frame.mask, Some([136, 35, 93, 205]));

            let mut dst = vec![0u8; frame.length];
            let remaining = decoder.read_frame_data(&frame, frame.length, &mut dst, 0);
            assert_eq!(remaining, 0);
            assert_eq!(String::from_utf8(dst).unwrap(), "over9000");
        }
        other => panic!("expected text frame, got {other:?}"),
    }

    let mut decoder = WsFrameDecoder::new();
    decoder.push(vec![129, 8, 111, 118, 101, 114, 57, 48, 48, 48]);
    match read_frame(&mut decoder) {
        WsFrame::Data(frame) => {
            assert!(frame.fin);
            assert_eq!(frame.opcode, WsFrameOpcode::Text as u8);
            assert_eq!(frame.length, 8);
            assert!(frame.mask.is_none());

            let mut dst = vec![0u8; frame.length];
            let remaining = decoder.read_frame_data(&frame, frame.length, &mut dst, 0);
            assert_eq!(remaining, 0);
            assert_eq!(String::from_utf8(dst).unwrap(), "over9000");
        }
        other => panic!("expected text frame, got {other:?}"),
    }
}

#[test]
fn ws_decoder_invalid_control_and_partial_payload_matrix() {
    let mut decoder = WsFrameDecoder::new();
    // ping with payload length 126 (invalid control frame length)
    decoder.push(vec![0x89, 0x7e, 0x00, 0x7e]);
    let err = decoder.read_frame_header().unwrap_err();
    assert_eq!(
        err,
        json_joy_json_pack::ws::WsFrameDecodingError::InvalidFrame
    );

    let mut decoder = WsFrameDecoder::new();
    // Header says ping payload length is 3, but only one payload octet is available.
    decoder.push(vec![0x89, 0x03, 0x41]);
    // Upstream behavior: return undefined / need-more-data, not a hard failure.
    let partial = decoder
        .read_frame_header()
        .expect("partial frame should not error");
    assert!(partial.is_none());
}
