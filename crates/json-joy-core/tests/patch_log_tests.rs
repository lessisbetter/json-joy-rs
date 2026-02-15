use json_joy_core::patch::Patch;
use json_joy_core::patch_log::{
    append_patch, deserialize_patches, serialize_patches, PatchLogError, MAX_PATCH_SIZE,
};

fn decode_hex(s: &str) -> Vec<u8> {
    assert!(s.len() % 2 == 0, "hex string must have even length");
    let mut out = Vec::with_capacity(s.len() / 2);
    let bytes = s.as_bytes();
    for i in (0..bytes.len()).step_by(2) {
        let hi = (bytes[i] as char).to_digit(16).expect("invalid hex") as u8;
        let lo = (bytes[i + 1] as char).to_digit(16).expect("invalid hex") as u8;
        out.push((hi << 4) | lo);
    }
    out
}

fn sample_patch() -> Patch {
    // From fixture: diff_root_number_to_number_v1 patch bytes.
    let bytes = decode_hex("c1b20403f702000248800003");
    Patch::from_binary(&bytes).expect("sample patch should decode")
}

#[test]
fn serialize_deserialize_empty_roundtrip() {
    let bin = serialize_patches(&[]);
    assert!(bin.is_empty());

    let decoded = deserialize_patches(&bin).expect("empty log should decode");
    assert!(decoded.is_empty());
}

#[test]
fn serialize_deserialize_multiple_roundtrip() {
    let p1 = sample_patch();
    let p2 = sample_patch();

    let bin = serialize_patches(&[p1.clone(), p2.clone()]);
    let decoded = deserialize_patches(&bin).expect("patch log should decode");

    assert_eq!(decoded.len(), 2);
    assert_eq!(decoded[0].to_binary(), p1.to_binary());
    assert_eq!(decoded[1].to_binary(), p2.to_binary());
}

#[test]
fn append_patch_empty_and_existing() {
    let p1 = sample_patch();
    let p2 = sample_patch();

    let log1 = append_patch(&[], &p1);
    let decoded1 = deserialize_patches(&log1).expect("single append should decode");
    assert_eq!(decoded1.len(), 1);

    let log2 = append_patch(&log1, &p2);
    let decoded2 = deserialize_patches(&log2).expect("double append should decode");
    assert_eq!(decoded2.len(), 2);
}

#[test]
fn reject_unsupported_version() {
    let bad = vec![0x00, 0x01];
    let err = deserialize_patches(&bad).expect_err("must reject unsupported version");
    assert!(matches!(err, PatchLogError::UnsupportedVersion(0)));
}

#[test]
fn reject_truncated_length_header() {
    let bad = vec![0x01, 0x00, 0x01];
    let err = deserialize_patches(&bad).expect_err("must reject truncated length header");
    assert!(matches!(err, PatchLogError::TruncatedLengthHeader));
}

#[test]
fn reject_length_exceeding_max() {
    let mut buf = vec![0x01, 0, 0, 0, 0];
    let too_big = (MAX_PATCH_SIZE as u32).saturating_add(1);
    buf[1..5].copy_from_slice(&too_big.to_be_bytes());

    let err = deserialize_patches(&buf).expect_err("must reject oversized patch length");
    assert!(matches!(err, PatchLogError::PatchTooLarge(_)));
}

#[test]
fn reject_truncated_patch_data() {
    let mut buf = vec![0x01, 0, 0, 0, 100];
    buf.extend_from_slice(&[0u8; 4]);

    let err = deserialize_patches(&buf).expect_err("must reject truncated patch data");
    assert!(matches!(err, PatchLogError::TruncatedPatchData));
}

#[test]
fn propagate_patch_decode_error() {
    // version=1, length=7, payload = ascii json bytes that fixture shows as decode error
    let payload = b"{\"x\":1}";
    let mut buf = vec![0x01];
    buf.extend_from_slice(&(payload.len() as u32).to_be_bytes());
    buf.extend_from_slice(payload);

    let err = deserialize_patches(&buf).expect_err("must propagate patch decode error");
    assert!(matches!(err, PatchLogError::PatchDecode(_)));
}
