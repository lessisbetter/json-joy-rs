//! Tests for binary base64 encoding (to_base64_bin).

use json_joy_base64::{create_to_base64_bin, create_to_base64_bin_uint8, to_base64, to_base64_bin};
use rand::Rng;

fn generate_blob() -> Vec<u8> {
    let mut rng = rand::thread_rng();
    let length = rng.gen_range(1..=100);
    (0..length).map(|_| rng.gen::<u8>()).collect()
}

fn copy_slice(arr: &[u8]) -> Vec<u8> {
    arr.to_vec()
}

#[test]
fn works() {
    let encode = create_to_base64_bin(None, None).unwrap();
    let encode_uint8 = create_to_base64_bin_uint8(None, None).unwrap();
    let encode_no_padding = create_to_base64_bin(None, Some("")).unwrap();

    for _ in 0..100 {
        let blob = generate_blob();
        let expected = to_base64(&blob);
        let expected_bytes = expected.as_bytes();

        // Test with DataView-style encoding
        let mut bin_with_buffer = vec![0u8; blob.len() * 4 + 3];
        let len = encode(&blob, 0, blob.len(), &mut bin_with_buffer, 3);
        let encoded = &bin_with_buffer[3..len];
        assert_eq!(
            encoded,
            expected_bytes,
            "Failed for blob of length {}",
            blob.len()
        );

        // Verify no mutation of input
        let dupe = copy_slice(&blob);
        let _ = encode_no_padding(&blob, 0, blob.len(), &mut bin_with_buffer, 3);
        assert_eq!(dupe, blob);

        // Test with Uint8-style encoding
        let dupe2 = copy_slice(&blob);
        let mut bin_with_buffer2 = vec![0u8; blob.len() * 4 + 3];
        let len2 = encode_uint8(&blob, 0, blob.len(), &mut bin_with_buffer2, 3);
        assert_eq!(dupe2, blob);
        let encoded2 = &bin_with_buffer2[3..len2];
        assert_eq!(encoded2, expected_bytes);
    }
}

#[test]
fn empty_input() {
    let mut dest = vec![0u8; 100];
    let len = to_base64_bin(b"", 0, 0, &mut dest, 0);
    assert_eq!(len, 0);
}

#[test]
fn with_offset() {
    let data = b"hello";
    let mut dest = vec![0u8; 100];
    let len = to_base64_bin(data, 0, data.len(), &mut dest, 10);
    assert_eq!(&dest[10..len], b"aGVsbG8=");
}
