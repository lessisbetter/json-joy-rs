use json_joy_json_pack::rm::{RmRecordDecoder, RmRecordEncoder};

fn header_value(bytes: &[u8]) -> u32 {
    u32::from_be_bytes([bytes[0], bytes[1], bytes[2], bytes[3]])
}

#[test]
fn rm_encoder_header_matrix() {
    let mut encoder = RmRecordEncoder::new();

    let result = encoder.encode_hdr(true, 0);
    assert_eq!(result, vec![0x80, 0x00, 0x00, 0x00]);

    let result = encoder.encode_hdr(false, 0);
    assert_eq!(result, vec![0x00, 0x00, 0x00, 0x00]);

    let result = encoder.encode_hdr(true, 100);
    assert_eq!(result.len(), 4);
    let value = header_value(&result);
    assert_ne!(value & 0x8000_0000, 0);
    assert_eq!(value & 0x7fff_ffff, 100);

    let result = encoder.encode_hdr(false, 1000);
    assert_eq!(result.len(), 4);
    let value = header_value(&result);
    assert_eq!(value & 0x8000_0000, 0);
    assert_eq!(value & 0x7fff_ffff, 1000);

    let max_length = 0x7fff_ffff;
    let result = encoder.encode_hdr(true, max_length);
    assert_eq!(result.len(), 4);
    let value = header_value(&result);
    assert_ne!(value & 0x8000_0000, 0);
    assert_eq!(value & 0x7fff_ffff, max_length);
}

#[test]
fn rm_encoder_record_matrix() {
    let mut encoder = RmRecordEncoder::new();

    let record: [u8; 0] = [];
    let result = encoder.encode_record(&record);
    assert_eq!(result.len(), 4);
    let header = header_value(&result);
    assert_ne!(header & 0x8000_0000, 0);
    assert_eq!(header & 0x7fff_ffff, 0);

    let record = [0x42];
    let result = encoder.encode_record(&record);
    assert_eq!(result.len(), 5);
    let header = header_value(&result);
    assert_ne!(header & 0x8000_0000, 0);
    assert_eq!(header & 0x7fff_ffff, 1);
    assert_eq!(result[4], 0x42);

    let record = [1, 2, 3, 4, 5];
    let result = encoder.encode_record(&record);
    assert_eq!(result.len(), 9);
    let header = header_value(&result);
    assert_ne!(header & 0x8000_0000, 0);
    assert_eq!(header & 0x7fff_ffff, 5);
    assert_eq!(result[4..], [1, 2, 3, 4, 5]);

    let record = b"hello";
    let result = encoder.encode_record(record);
    assert_eq!(result.len(), 9);
    let header = header_value(&result);
    assert_ne!(header & 0x8000_0000, 0);
    assert_eq!(header & 0x7fff_ffff, 5);
    assert_eq!(&result[4..], record);

    let size = 10_000usize;
    let mut record = vec![0u8; size];
    for (i, byte) in record.iter_mut().enumerate() {
        *byte = (i % 256) as u8;
    }
    let result = encoder.encode_record(&record);
    assert_eq!(result.len(), 4 + size);
    let header = header_value(&result);
    assert_ne!(header & 0x8000_0000, 0);
    assert_eq!(header & 0x7fff_ffff, size as u32);
    assert_eq!(result[4..], record);
}

#[test]
fn rm_encoder_write_hdr_and_write_record_matrix() {
    let mut encoder = RmRecordEncoder::new();

    encoder.write_hdr(true, 42);
    encoder.write_hdr(false, 100);
    let result = encoder.writer.flush();
    assert_eq!(result.len(), 8);

    let record1 = [1, 2, 3];
    let record2 = [4, 5];
    encoder.write_record(&record1);
    encoder.write_record(&record2);
    let result = encoder.writer.flush();
    assert_eq!(result.len(), 4 + record1.len() + 4 + record2.len());
}

#[test]
fn rm_encoder_write_fragment_matrix() {
    let mut encoder = RmRecordEncoder::new();

    let record = [1, 2, 3, 4, 5, 6, 7, 8];
    encoder.write_fragment(&record, 2, 3, false);
    let result = encoder.writer.flush();
    assert_eq!(result.len(), 7);
    let header = header_value(&result);
    assert_eq!(header & 0x8000_0000, 0);
    assert_eq!(header & 0x7fff_ffff, 3);
    assert_eq!(result[4..], [3, 4, 5]);

    let record = [10, 20, 30];
    encoder.write_fragment(&record, 0, 3, true);
    let result = encoder.writer.flush();
    assert_eq!(result.len(), 7);
    let header = header_value(&result);
    assert_ne!(header & 0x8000_0000, 0);
    assert_eq!(header & 0x7fff_ffff, 3);
    assert_eq!(result[4..], [10, 20, 30]);
}

#[test]
fn rm_decoder_basic_record_matrix() {
    let mut decoder = RmRecordDecoder::new();
    assert_eq!(decoder.read_record(), None);

    decoder.push(&[0, 0, 0, 0]);
    assert_eq!(decoder.read_record(), None);

    let mut decoder = RmRecordDecoder::new();
    decoder.push(&[0, 0, 0, 0, 0]);
    assert_eq!(decoder.read_record(), None);

    let mut decoder = RmRecordDecoder::new();
    decoder.push(&[0]);
    assert_eq!(decoder.read_record(), None);
    decoder.push(&[0]);
    assert_eq!(decoder.read_record(), None);
    decoder.push(&[0]);
    assert_eq!(decoder.read_record(), None);
    decoder.push(&[0]);
    assert_eq!(decoder.read_record(), None);

    let mut decoder = RmRecordDecoder::new();
    decoder.push(&[0x80, 0, 0, 1, 42]);
    assert_eq!(decoder.read_record(), Some(vec![42]));

    let data = vec![1, 2, 3, 4, 5];
    let mut decoder = RmRecordDecoder::new();
    let mut payload = vec![0x80, 0, 0, data.len() as u8];
    payload.extend_from_slice(&data);
    decoder.push(&payload);
    assert_eq!(decoder.read_record(), Some(data));

    let data = b"hello world";
    let mut decoder = RmRecordDecoder::new();
    let mut payload = vec![0x80, 0, 0, data.len() as u8];
    payload.extend_from_slice(data);
    decoder.push(&payload);
    assert_eq!(decoder.read_record(), Some(data.to_vec()));

    let size = 10_000usize;
    let mut data = vec![0u8; size];
    for (i, byte) in data.iter_mut().enumerate() {
        *byte = (i % 256) as u8;
    }
    let mut decoder = RmRecordDecoder::new();
    let mut payload = vec![
        0x80,
        ((size >> 16) & 0xff) as u8,
        ((size >> 8) & 0xff) as u8,
        (size & 0xff) as u8,
    ];
    payload.extend_from_slice(&data);
    decoder.push(&payload);
    assert_eq!(decoder.read_record(), Some(data));
}

#[test]
fn rm_decoder_streamed_byte_matrix() {
    let mut decoder = RmRecordDecoder::new();
    assert_eq!(decoder.read_record(), None);

    decoder.push(&[0b1000_0000]);
    assert_eq!(decoder.read_record(), None);
    decoder.push(&[0]);
    assert_eq!(decoder.read_record(), None);
    decoder.push(&[0]);
    assert_eq!(decoder.read_record(), None);
    decoder.push(&[1]);
    assert_eq!(decoder.read_record(), None);
    decoder.push(&[42]);
    assert_eq!(decoder.read_record(), Some(vec![42]));
    assert_eq!(decoder.read_record(), None);

    decoder.push(&[0b1000_0000, 0, 0]);
    assert_eq!(decoder.read_record(), None);
    assert_eq!(decoder.read_record(), None);
    decoder.push(&[1, 43]);
    assert_eq!(decoder.read_record(), Some(vec![43]));
    assert_eq!(decoder.read_record(), None);
}

#[test]
fn rm_decoder_fragmented_record_matrix() {
    let mut decoder = RmRecordDecoder::new();
    let part1 = [1, 2, 3];
    let part2 = [4, 5, 6];

    let mut fragment = vec![0x00, 0, 0, part1.len() as u8];
    fragment.extend_from_slice(&part1);
    decoder.push(&fragment);
    assert_eq!(decoder.read_record(), None);

    let mut fragment = vec![0x80, 0, 0, part2.len() as u8];
    fragment.extend_from_slice(&part2);
    decoder.push(&fragment);
    assert_eq!(decoder.read_record(), Some(vec![1, 2, 3, 4, 5, 6]));

    let mut decoder = RmRecordDecoder::new();
    let part1 = [1, 2];
    let part2 = [3, 4];
    let part3 = [5, 6];

    let mut fragment = vec![0x00, 0, 0, part1.len() as u8];
    fragment.extend_from_slice(&part1);
    decoder.push(&fragment);
    assert_eq!(decoder.read_record(), None);

    let mut fragment = vec![0x00, 0, 0, part2.len() as u8];
    fragment.extend_from_slice(&part2);
    decoder.push(&fragment);
    assert_eq!(decoder.read_record(), None);

    let mut fragment = vec![0x80, 0, 0, part3.len() as u8];
    fragment.extend_from_slice(&part3);
    decoder.push(&fragment);
    assert_eq!(decoder.read_record(), Some(vec![1, 2, 3, 4, 5, 6]));
}
