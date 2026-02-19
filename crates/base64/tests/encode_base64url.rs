//! Tests for URL-safe base64 encoding (to_base64_url).

use json_joy_base64::to_base64_url;
use rand::Rng;

fn generate_blob() -> Vec<u8> {
    let mut rng = rand::thread_rng();
    let length = rng.gen_range(1..=100);
    (0..length).map(|_| rng.gen::<u8>()).collect()
}

#[test]
fn works() {
    for _ in 0..100 {
        let blob = generate_blob();
        let base64url = to_base64_url(&blob, blob.len());

        // Verify it's URL-safe (no +, /, or =)
        assert!(!base64url.contains('+'));
        assert!(!base64url.contains('/'));
        assert!(!base64url.contains('='));

        // Convert to standard base64 and verify
        let standard = base64url.replace('-', "+").replace('_', "/");

        // Add padding if needed
        let standard = match standard.len() % 4 {
            2 => format!("{}==", standard),
            3 => format!("{}=", standard),
            _ => standard,
        };

        // Verify it matches standard base64 encoding
        let expected = base64_encode(&blob);
        assert_eq!(
            standard,
            expected,
            "Failed for blob of length {}",
            blob.len()
        );
    }
}

#[test]
fn empty_input() {
    assert_eq!(to_base64_url(b"", 0), "");
}

#[test]
fn single_byte() {
    assert_eq!(to_base64_url(b"f", 1), "Zg");
}

#[test]
fn two_bytes() {
    assert_eq!(to_base64_url(b"fo", 2), "Zm8");
}

#[test]
fn three_bytes() {
    assert_eq!(to_base64_url(b"foo", 3), "Zm9v");
}

/// Simple base64 encoding for test verification
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
