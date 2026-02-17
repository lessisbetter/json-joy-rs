use json_joy_json_pointer::get;
use serde_json::json;

#[test]
fn test_upstream_numeric_path_compatibility() {
    let doc = json!([1, 2, 3]);

    // Upstream TypeScript supports both string and numeric path steps
    // In TypeScript: PathStep = string | number
    // In Rust we only have strings - this is a design difference

    // With string indices (current implementation)
    assert_eq!(get(&doc, &["0".to_string()]), Some(&json!(1)));
    assert_eq!(get(&doc, &["1".to_string()]), Some(&json!(2)));
}
