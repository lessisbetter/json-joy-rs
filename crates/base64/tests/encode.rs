//! Tests for base64 encoding (to_base64).

use json_joy_base64::{create_to_base64, to_base64};
use rand::Rng;

fn generate_blob() -> Vec<u8> {
    let mut rng = rand::thread_rng();
    let length = rng.gen_range(1..=100);
    (0..length).map(|_| rng.gen::<u8>()).collect()
}

#[test]
fn works() {
    let encode2 = create_to_base64(None, None).unwrap();

    for _ in 0..100 {
        let blob = generate_blob();
        let result = to_base64(&blob);
        let result2 = encode2(&blob, blob.len());

        // Verify against known-good encoding using the base64 crate
        let expected = base64_encode(&blob);
        assert_eq!(result, expected, "Failed for blob of length {}", blob.len());
        assert_eq!(
            result2,
            expected,
            "Failed for blob of length {}",
            blob.len()
        );
    }
}

#[test]
fn empty_input() {
    assert_eq!(to_base64(b""), "");
}

#[test]
fn single_byte() {
    assert_eq!(to_base64(b"f"), "Zg==");
}

#[test]
fn two_bytes() {
    assert_eq!(to_base64(b"fo"), "Zm8=");
}

#[test]
fn three_bytes() {
    assert_eq!(to_base64(b"foo"), "Zm9v");
}

#[test]
fn hello_world() {
    assert_eq!(to_base64(b"hello world"), "aGVsbG8gd29ybGQ=");
}

/// Simple base64 encoding for test verification (no external dependency)
fn base64_encode(data: &[u8]) -> String {
    const ALPHABET: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";

    let mut result = String::new();
    let mut i = 0;

    while i < data.len() {
        let chunk = &data[i..std::cmp::min(i + 3, data.len())];
        let b0 = chunk[0];
        let b1 = chunk.get(1).copied().unwrap_or(0);
        let b2 = chunk.get(2).copied().unwrap_or(0);

        result.push(ALPHABET[(b0 >> 2) as usize] as char);
        result.push(ALPHABET[(((b0 & 0x03) << 4) | (b1 >> 4)) as usize] as char);

        if chunk.len() > 1 {
            result.push(ALPHABET[(((b1 & 0x0f) << 2) | (b2 >> 6)) as usize] as char);
        } else {
            result.push('=');
        }

        if chunk.len() > 2 {
            result.push(ALPHABET[(b2 & 0x3f) as usize] as char);
        } else {
            result.push('=');
        }

        i += 3;
    }

    result
}
