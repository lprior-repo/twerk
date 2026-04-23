use twerk_common::conf::parsing::flatten_table;
use twerk_common::conf::types::ConfigState;

#[kani::proof]
fn config_state_get_str_none_for_missing_key() {
    let state = ConfigState::new();
    assert!(state.get_str("nonexistent").is_none());
}

#[kani::proof]
fn config_state_get_bool_from_string_true() {
    let mut state = ConfigState::new();
    state.insert("flag".to_string(), toml::Value::String("true".to_string()));
    assert_eq!(state.get_bool("flag"), Some(true));
}

#[kani::proof]
fn config_state_get_bool_from_string_false() {
    let mut state = ConfigState::new();
    state.insert("flag".to_string(), toml::Value::String("false".to_string()));
    assert_eq!(state.get_bool("flag"), Some(false));
}

#[kani::proof]
fn flatten_table_preserves_leaf_count() {
    // Build a simple nested TOML table:
    //   [section]
    //   key1 = "val1"
    //   key2 = "val2"
    let mut section = toml::value::Table::new();
    section.insert("key1".to_string(), toml::Value::String("val1".to_string()));
    section.insert("key2".to_string(), toml::Value::String("val2".to_string()));

    let mut root = toml::value::Table::new();
    root.insert("section".to_string(), toml::Value::Table(section));

    let flat = flatten_table("", &root);
    assert_eq!(flat.len(), 2, "two leaf values should produce two flat entries");
    assert!(flat.contains_key("section.key1"));
    assert!(flat.contains_key("section.key2"));
}
