//! Tests for base64 decoding (from_base64).

use json_joy_base64::{create_from_base64, from_base64, to_base64, Base64Error};
use rand::Rng;

fn generate_blob() -> Vec<u8> {
    let mut rng = rand::thread_rng();
    let length = rng.gen_range(0..=100);
    (0..length).map(|_| rng.gen::<u8>()).collect()
}

#[test]
fn works() {
    let from_base64_2 = create_from_base64(None, false).unwrap();

    for _ in 0..100 {
        let blob = generate_blob();
        let encoded = to_base64(&blob);
        let decoded1 = from_base64_2(&encoded).unwrap();
        let decoded2 = from_base64(&encoded).unwrap();
        assert_eq!(decoded1, blob);
        assert_eq!(decoded2, blob);
    }
}

#[test]
fn handles_invalid_values() {
    for _ in 0..100 {
        let blob = generate_blob();
        let encoded = to_base64(&blob);
        let invalid = format!("{}!!!!", encoded);
        let result = from_base64(&invalid);
        assert!(matches!(result, Err(Base64Error::InvalidBase64String)));
    }
}

#[test]
fn empty_input() {
    assert_eq!(from_base64("").unwrap(), b"");
}

#[test]
fn single_byte() {
    assert_eq!(from_base64("Zg==").unwrap(), b"f");
}

#[test]
fn two_bytes() {
    assert_eq!(from_base64("Zm8=").unwrap(), b"fo");
}

#[test]
fn three_bytes() {
    assert_eq!(from_base64("Zm9v").unwrap(), b"foo");
}

#[test]
fn hello_world() {
    assert_eq!(from_base64("aGVsbG8gd29ybGQ=").unwrap(), b"hello world");
}
