//! Tests for URL-safe base64 decoding (from_base64_url).

use json_joy_base64::{from_base64_url, to_base64};
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
        let encoded = to_base64(&blob)
            .replace('+', "-")
            .replace('/', "_")
            .trim_end_matches('=')
            .to_string();

        let decoded = from_base64_url(&encoded).unwrap();
        assert_eq!(decoded, blob);
    }
}

#[test]
fn empty_input() {
    assert_eq!(from_base64_url("").unwrap(), b"");
}

#[test]
fn hello_world() {
    // "hello world" encoded as base64url without padding
    let decoded = from_base64_url("aGVsbG8gd29ybGQ").unwrap();
    assert_eq!(decoded, b"hello world");
}

#[test]
fn single_byte() {
    // "f" encoded as base64url without padding
    assert_eq!(from_base64_url("Zg").unwrap(), b"f");
}

#[test]
fn two_bytes() {
    // "fo" encoded as base64url without padding
    assert_eq!(from_base64_url("Zm8").unwrap(), b"fo");
}
