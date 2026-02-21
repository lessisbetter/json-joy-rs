use json_joy_json_pack::ion::{system_symbol_import, system_symbol_table, Import};

#[test]
fn ion_import_instantiation_matrix() {
    let system = system_symbol_import();
    let base = system_symbol_table().len();
    let imp = Import::new(Some(system), vec!["foo".into(), "bar".into()]);

    assert_eq!(imp.get_id("foo"), Some(base + 1));
    assert_eq!(imp.get_id("bar"), Some(base + 2));
    assert_eq!(imp.get_text(base + 1), Some("foo"));
    assert_eq!(imp.get_text(base + 2), Some("bar"));
}

#[test]
fn ion_import_add_symbols_matrix() {
    let system = system_symbol_import();
    let base = system_symbol_table().len();
    let mut imp = Import::new(Some(system), vec!["foo".into(), "bar".into()]);

    assert_eq!(imp.add("baz"), base + 3);
    assert_eq!(imp.add("__proto__"), base + 4);
    assert_eq!(imp.get_text(base + 3), Some("baz"));
    assert_eq!(imp.get_text(base + 4), Some("__proto__"));
}

#[test]
fn ion_import_duplicate_symbol_id_matrix() {
    let system = system_symbol_import();
    let base = system_symbol_table().len();
    let mut imp = Import::new(Some(system), vec![]);

    let id1 = imp.add("baz");
    let id2 = imp.add("bar");
    let id3 = imp.add("baz");

    assert_eq!(id1, base + 1);
    assert_eq!(id3, id1);
    assert_ne!(id1, id2);
    assert_eq!(imp.add("bar"), id2);
}
