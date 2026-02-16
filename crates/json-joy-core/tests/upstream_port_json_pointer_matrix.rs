use json_joy_core::json_pointer::{
    escape_component, format_json_pointer, parse_json_pointer, parse_json_pointer_relaxed,
    unescape_component,
};

#[test]
fn upstream_port_json_pointer_parse_matrix() {
    assert_eq!(parse_json_pointer("").unwrap(), Vec::<String>::new());
    assert_eq!(parse_json_pointer("/").unwrap(), vec![String::new()]);
    assert_eq!(
        parse_json_pointer("/foo/bar").unwrap(),
        vec!["foo".to_string(), "bar".to_string()]
    );
    assert_eq!(
        parse_json_pointer("/foo///").unwrap(),
        vec![
            "foo".to_string(),
            String::new(),
            String::new(),
            String::new()
        ]
    );
    assert_eq!(
        parse_json_pointer("/a~0b/c~1d/1").unwrap(),
        vec!["a~b".to_string(), "c/d".to_string(), "1".to_string()]
    );
}

#[test]
fn upstream_port_json_pointer_escape_unescape_matrix() {
    assert_eq!(unescape_component("foobar"), "foobar");
    assert_eq!(unescape_component("foo~0~1"), "foo~/");
    assert_eq!(escape_component("foobar"), "foobar");
    assert_eq!(escape_component("foo~/"), "foo~0~1");
}

#[test]
fn upstream_port_json_pointer_format_matrix() {
    assert_eq!(format_json_pointer(&[]), "");
    assert_eq!(format_json_pointer(&[String::new()]), "/");
    assert_eq!(
        format_json_pointer(&["foo".to_string(), "bar".to_string()]),
        "/foo/bar"
    );
    assert_eq!(
        format_json_pointer(&["a~b".to_string(), "c/d".to_string(), "1".to_string()]),
        "/a~0b/c~1d/1"
    );
}

#[test]
fn upstream_port_json_pointer_relaxed_parse_matrix() {
    assert_eq!(
        parse_json_pointer_relaxed("foo/bar").unwrap(),
        vec!["foo".to_string(), "bar".to_string()]
    );
    assert_eq!(
        parse_json_pointer_relaxed("/foo/bar").unwrap(),
        vec!["foo".to_string(), "bar".to_string()]
    );
}
