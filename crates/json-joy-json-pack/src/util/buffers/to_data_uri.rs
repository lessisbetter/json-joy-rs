//! Convert raw bytes to a data URI.
//!
//! Upstream reference: `json-pack/src/util/buffers/toDataUri.ts`

/// Convert bytes to `data:application/octet-stream;base64,...`.
///
/// Optional parameters are appended as `;key=value` before the comma.
pub fn to_data_uri(buf: &[u8], params: &[(String, String)]) -> String {
    let mut uri = String::from("data:application/octet-stream;base64");
    for (key, value) in params {
        uri.push(';');
        uri.push_str(key);
        uri.push('=');
        uri.push_str(value);
    }
    uri.push(',');
    uri.push_str(&json_joy_base64::to_base64(buf));
    uri
}
