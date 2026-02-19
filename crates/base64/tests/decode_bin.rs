//! Tests for binary base64 decoding (from_base64_bin).

use json_joy_base64::{from_base64_bin, to_base64_bin};
use rand::Rng;

fn generate_blob() -> Vec<u8> {
    let mut rng = rand::thread_rng();
    let length = rng.gen_range(0..=100);
    (0..length).map(|_| rng.gen::<u8>()).collect()
}

#[test]
fn works() {
    for _ in 0..100 {
        let blob = generate_blob();
        let mut dest = vec![0u8; blob.len() * 4];
        let length = to_base64_bin(&blob, 0, blob.len(), &mut dest, 0);
        let encoded = &dest[..length];

        // Decode with full padding
        let decoded = from_base64_bin(encoded, 0, length).unwrap();
        assert_eq!(decoded, blob);

        // Decode with missing padding (if there was padding)
        let padding = if length > 0 && dest[length - 1] == b'=' {
            1
        } else {
            0
        } + if length > 1 && dest[length - 2] == b'=' {
            1
        } else {
            0
        };

        if padding > 0 {
            let decoded2 = from_base64_bin(encoded, 0, length - padding).unwrap();
            assert_eq!(decoded2, blob);
        }
    }
}

#[test]
fn empty_input() {
    let result = from_base64_bin(b"", 0, 0).unwrap();
    assert_eq!(result, b"");
}

#[test]
fn with_offset() {
    let encoded = b"xxxxaGVsbG8="; // "xxxx" prefix, then "hello" encoded
    let decoded = from_base64_bin(encoded, 4, 8).unwrap();
    assert_eq!(decoded, b"hello");
}
