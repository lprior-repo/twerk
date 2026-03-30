#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::get_first
)]

use std::collections::HashMap;
use std::env;
use std::fs;
use std::path::Path;

use tempfile::TempDir;

use crate::conf::env::extract_env_vars;
use crate::conf::parsing::{expand_path, flatten_table, merge_values, parse_toml_file};
use crate::conf::types::{ConfigError, ConfigState};

fn reset_env() {
    let keys: Vec<String> = env::vars()
        .filter(|(k, _)| k.starts_with("TWERK_"))
        .map(|(k, _)| k)
        .collect();
    for key in keys {
        env::remove_var(key);
    }
}

struct TestFixture {
    #[allow(dead_code)]
    temp_dir: TempDir,
}

impl TestFixture {
    fn new() -> Self {
        reset_env();
        let temp_dir = TempDir::new().expect("tempdir");
        Self { temp_dir }
    }

    fn path(&self, name: &str) -> std::path::PathBuf {
        self.temp_dir.path().join(name)
    }
}

impl Drop for TestFixture {
    fn drop(&mut self) {
        reset_env();
    }
}

#[test]
fn test_expand_path_tilde() {
    let path = expand_path("~/test");
    let home = dirs::home_dir().expect("home dir");
    assert_eq!(path, home.join("test"));
}

#[test]
fn test_expand_path_absolute() {
    let path = expand_path("/etc/config.toml");
    assert_eq!(path, Path::new("/etc/config.toml"));
}

#[test]
fn test_parse_toml_file_not_exist() {
    let result = parse_toml_file("/nonexistent/path.toml");
    assert!(result.is_err());
    if let Err(ConfigError::NotFound(p)) = result {
        assert_eq!(p, "/nonexistent/path.toml");
    } else {
        panic!("expected NotFound error");
    }
}

#[test]
fn test_parse_toml_file_bad_contents() {
    let fixture = TestFixture::new();
    let path = fixture.path("bad.toml");
    fs::write(&path, "xyz").expect("write");

    let result = parse_toml_file(path.to_str().unwrap());
    assert!(result.is_err());
    if let Err(ConfigError::ParseError { .. }) = result {
    } else {
        panic!("expected ParseError");
    }
}

#[test]
fn test_parse_toml_file_valid() {
    let fixture = TestFixture::new();
    let path = fixture.path("valid.toml");
    fs::write(&path, "[main]\nkey1 = \"value1\"").expect("write");

    let result = parse_toml_file(path.to_str().unwrap()).expect("parse");
    assert!(result.is_table());
}

#[test]
fn test_parse_toml_file_with_integer() {
    let fixture = TestFixture::new();
    let path = fixture.path("config.toml");
    fs::write(&path, "[settings]\ncount = 42").expect("write");

    let result = parse_toml_file(path.to_str().unwrap()).expect("parse");
    let table = result.as_table().expect("should be table");
    let count = table
        .get("settings")
        .and_then(|t| t.as_table())
        .and_then(|t| t.get("count"))
        .and_then(|v| v.as_integer());
    assert_eq!(count, Some(42));
}

#[test]
fn test_parse_toml_file_with_bool() {
    let fixture = TestFixture::new();
    let path = fixture.path("config.toml");
    fs::write(&path, "[settings]\nenabled = true").expect("write");

    let result = parse_toml_file(path.to_str().unwrap()).expect("parse");
    let table = result.as_table().expect("should be table");
    let enabled = table
        .get("settings")
        .and_then(|t| t.as_table())
        .and_then(|t| t.get("enabled"))
        .and_then(|v| v.as_bool());
    assert_eq!(enabled, Some(true));
}

#[test]
fn test_parse_toml_file_with_array() {
    let fixture = TestFixture::new();
    let path = fixture.path("config.toml");
    fs::write(&path, "[settings]\nvalues = [\"a\", \"b\", \"c\"]").expect("write");

    let result = parse_toml_file(path.to_str().unwrap()).expect("parse");
    let table = result.as_table().expect("should be table");
    let values = table
        .get("settings")
        .and_then(|t| t.as_table())
        .and_then(|t| t.get("values"))
        .and_then(|v| v.as_array())
        .map(|arr| arr.iter().filter_map(|v| v.as_str()).collect::<Vec<_>>());
    assert_eq!(values, Some(vec!["a", "b", "c"]));
}

#[test]
fn test_flatten_table() {
    let mut table = toml::value::Table::new();
    table.insert(
        "key1".to_string(),
        toml::Value::String("value1".to_string()),
    );
    table.insert(
        "nested".to_string(),
        toml::Value::Table({
            let mut nested = toml::value::Table::new();
            nested.insert("key2".to_string(), toml::Value::Integer(42));
            nested
        }),
    );

    let flat = flatten_table("", &table);
    assert_eq!(flat.get("key1").and_then(|v| v.as_str()), Some("value1"));
    assert_eq!(
        flat.get("nested.key2").and_then(|v| v.as_integer()),
        Some(42)
    );
}

#[test]
fn test_flatten_table_with_prefix() {
    let mut table = toml::value::Table::new();
    table.insert(
        "key1".to_string(),
        toml::Value::String("value1".to_string()),
    );

    let flat = flatten_table("prefix", &table);
    assert!(flat.contains_key("prefix.key1"));
}

#[test]
fn test_flatten_table_nested_nested() {
    let mut table = toml::value::Table::new();
    table.insert(
        "a".to_string(),
        toml::Value::Table({
            let mut a = toml::value::Table::new();
            a.insert(
                "b".to_string(),
                toml::Value::Table({
                    let mut b = toml::value::Table::new();
                    b.insert("c".to_string(), toml::Value::String("deep".to_string()));
                    b
                }),
            );
            a
        }),
    );

    let flat = flatten_table("", &table);
    assert_eq!(flat.get("a.b.c").and_then(|v| v.as_str()), Some("deep"));
}

#[test]
fn test_merge_values() {
    let mut base = HashMap::new();
    base.insert("key1".to_string(), toml::Value::String("base1".to_string()));
    base.insert("key2".to_string(), toml::Value::String("base2".to_string()));

    let mut overrides = HashMap::new();
    overrides.insert(
        "key1".to_string(),
        toml::Value::String("override1".to_string()),
    );
    overrides.insert("key3".to_string(), toml::Value::String("new".to_string()));

    let merged = merge_values(base, overrides);
    assert_eq!(
        merged.get("key1").and_then(|v| v.as_str()),
        Some("override1")
    );
    assert_eq!(merged.get("key2").and_then(|v| v.as_str()), Some("base2"));
    assert_eq!(merged.get("key3").and_then(|v| v.as_str()), Some("new"));
}

#[test]
fn test_merge_values_override_wins() {
    let mut base = HashMap::new();
    base.insert("key".to_string(), toml::Value::Integer(1));

    let mut overrides = HashMap::new();
    overrides.insert("key".to_string(), toml::Value::Integer(2));

    let merged = merge_values(base, overrides);
    assert_eq!(merged.get("key").and_then(|v| v.as_integer()), Some(2));
}

#[test]
fn test_extract_env_vars_empty() {
    reset_env();
    let vars = extract_env_vars();
    assert!(vars.is_empty());
}

#[test]
fn test_extract_env_vars_with_twerk_prefix() {
    reset_env();
    env::set_var("TWERK_HELLO", "world");
    env::set_var("TWERK_MAIN_KEY1", "value1");
    env::set_var("TWERK_NESTED_KEY2", "value2");
    env::set_var("REGULAR_VAR", "should_be_ignored");

    let vars = extract_env_vars();

    assert_eq!(vars.get("hello").and_then(|v| v.as_str()), Some("world"));
    assert_eq!(
        vars.get("main.key1").and_then(|v| v.as_str()),
        Some("value1")
    );
    assert_eq!(
        vars.get("nested.key2").and_then(|v| v.as_str()),
        Some("value2")
    );
    assert!(!vars.contains_key("REGULAR_VAR"));

    // Clean up env vars for other tests
    reset_env();
}

#[test]
fn test_extract_env_vars_underscore_to_dot() {
    reset_env();
    env::set_var("TWERK_NESTED_DEEP_VALUE", "test");

    let vars = extract_env_vars();
    assert_eq!(
        vars.get("nested.deep.value").and_then(|v| v.as_str()),
        Some("test")
    );

    // Clean up env vars for other tests
    reset_env();
}

#[test]
fn test_config_state_get_str() {
    let mut state = ConfigState::new();
    state.insert(
        "key1".to_string(),
        toml::Value::String("value1".to_string()),
    );
    state.insert(
        "nested.key2".to_string(),
        toml::Value::String("value2".to_string()),
    );

    assert_eq!(state.get_str("key1"), Some("value1"));
    assert_eq!(state.get_str("nested.key2"), Some("value2"));
    assert_eq!(state.get_str("nonexistent"), None);
}

#[test]
fn test_config_state_get_bool() {
    let mut state = ConfigState::new();
    state.insert("bool_true".to_string(), toml::Value::Boolean(true));
    state.insert("bool_false".to_string(), toml::Value::Boolean(false));
    state.insert(
        "string_true".to_string(),
        toml::Value::String("true".to_string()),
    );
    state.insert(
        "string_false".to_string(),
        toml::Value::String("false".to_string()),
    );

    assert_eq!(state.get_bool("bool_true"), Some(true));
    assert_eq!(state.get_bool("bool_false"), Some(false));
    assert_eq!(state.get_bool("string_true"), Some(true));
    assert_eq!(state.get_bool("string_false"), Some(false));
    assert_eq!(state.get_bool("nonexistent"), None);
}

#[test]
fn test_config_state_get_int() {
    let mut state = ConfigState::new();
    state.insert("int_val".to_string(), toml::Value::Integer(42));
    state.insert(
        "string_int".to_string(),
        toml::Value::String("123".to_string()),
    );

    assert_eq!(state.get_int("int_val"), Some(42));
    assert_eq!(state.get_int("string_int"), Some(123));
    assert_eq!(state.get_int("nonexistent"), None);
}

#[test]
fn test_config_state_get_array() {
    let mut state = ConfigState::new();
    state.insert(
        "arr".to_string(),
        toml::Value::Array(vec![
            toml::Value::String("a".to_string()),
            toml::Value::String("b".to_string()),
        ]),
    );

    let arr = state.get_array("arr");
    assert_eq!(arr.map(|a| a.len()), Some(2));
    assert_eq!(
        arr.and_then(|a| a.get(0).and_then(|v| v.as_str())),
        Some("a")
    );
}

#[test]
fn test_config_state_get_table() {
    let mut state = ConfigState::new();
    state.insert(
        "section".to_string(),
        toml::Value::Table({
            let mut t = toml::value::Table::new();
            t.insert("key".to_string(), toml::Value::String("val".to_string()));
            t
        }),
    );

    let table = state.get_table("section");
    assert!(table.is_some());
    assert_eq!(
        table.and_then(|t| t.get("key").and_then(|v| v.as_str())),
        Some("val")
    );
}

#[test]
fn test_config_state_contains_key() {
    let mut state = ConfigState::new();
    state.insert(
        "exists".to_string(),
        toml::Value::String("value".to_string()),
    );

    assert!(state.contains_key("exists"));
    assert!(!state.contains_key("nonexistent"));
}

#[test]
fn test_config_state_string_map_for_key() {
    let mut state = ConfigState::new();
    state.insert(
        "mapping.key1".to_string(),
        toml::Value::String("value1".to_string()),
    );
    state.insert(
        "mapping.key2".to_string(),
        toml::Value::String("value2".to_string()),
    );

    let map = state.string_map_for_key("mapping");
    assert_eq!(map.get("key1"), Some(&"value1".to_string()));
    assert_eq!(map.get("key2"), Some(&"value2".to_string()));
}

#[test]
fn test_config_state_string_map_for_key_with_table() {
    let mut state = ConfigState::new();
    state.insert(
        "mapping".to_string(),
        toml::Value::Table({
            let mut t = toml::value::Table::new();
            t.insert(
                "key1".to_string(),
                toml::Value::String("value1".to_string()),
            );
            t
        }),
    );

    let map = state.string_map_for_key("mapping");
    assert_eq!(map.get("key1"), Some(&"value1".to_string()));
}

#[test]
fn test_config_state_int_map_for_key() {
    let mut state = ConfigState::new();
    state.insert("nums.one".to_string(), toml::Value::Integer(1));
    state.insert("nums.two".to_string(), toml::Value::Integer(2));

    let map = state.int_map_for_key("nums");
    assert_eq!(map.get("one"), Some(&1));
    assert_eq!(map.get("two"), Some(&2));
}

#[test]
fn test_config_state_int_map_for_key_with_string_values() {
    let mut state = ConfigState::new();
    state.insert(
        "nums.str_int".to_string(),
        toml::Value::String("42".to_string()),
    );

    let map = state.int_map_for_key("nums");
    assert_eq!(map.get("str_int"), Some(&42));
}

#[test]
fn test_config_state_bool_map_for_key() {
    let mut state = ConfigState::new();
    state.insert("flags.enabled".to_string(), toml::Value::Boolean(true));
    state.insert("flags.disabled".to_string(), toml::Value::Boolean(false));

    let map = state.bool_map_for_key("flags");
    assert_eq!(map.get("enabled"), Some(&true));
    assert_eq!(map.get("disabled"), Some(&false));
}

#[test]
fn test_config_state_bool_map_for_key_with_string_values() {
    let mut state = ConfigState::new();
    state.insert(
        "flags.str_true".to_string(),
        toml::Value::String("true".to_string()),
    );
    state.insert(
        "flags.str_false".to_string(),
        toml::Value::String("false".to_string()),
    );

    let map = state.bool_map_for_key("flags");
    assert_eq!(map.get("str_true"), Some(&true));
    assert_eq!(map.get("str_false"), Some(&false));
}

#[test]
fn test_config_state_strings_for_key() {
    let mut state = ConfigState::new();
    state.insert(
        "list".to_string(),
        toml::Value::Array(vec![
            toml::Value::String("a".to_string()),
            toml::Value::String("b".to_string()),
            toml::Value::String("c".to_string()),
        ]),
    );

    let strings = state.strings_for_key("list");
    assert_eq!(strings, vec!["a", "b", "c"]);
}

#[test]
fn test_config_state_strings_from_string_comma_separated() {
    let mut state = ConfigState::new();
    state.insert(
        "csv".to_string(),
        toml::Value::String("a, b, c".to_string()),
    );

    let strings = state.strings_from_string("csv");
    assert_eq!(strings, vec!["a", "b", "c"]);
}

#[test]
fn test_config_state_strings_for_key_or_string_prefers_array() {
    let mut state = ConfigState::new();
    state.insert(
        "values".to_string(),
        toml::Value::Array(vec![toml::Value::String("array_val".to_string())]),
    );

    let strings = state.strings_for_key_or_string("values");
    assert_eq!(strings, vec!["array_val"]);
}

#[test]
fn test_config_state_strings_for_key_or_string_falls_back_to_string() {
    let mut state = ConfigState::new();
    state.insert(
        "values".to_string(),
        toml::Value::String("comma,separated".to_string()),
    );

    let strings = state.strings_for_key_or_string("values");
    assert_eq!(strings, vec!["comma", "separated"]);
}

#[test]
fn test_config_state_build_table_from_flat() {
    let mut state = ConfigState::new();
    state.insert(
        "section.key1".to_string(),
        toml::Value::String("value1".to_string()),
    );
    state.insert("section.key2".to_string(), toml::Value::Integer(42));

    let table = state.build_table_from_flat("section");
    assert_eq!(table.get("key1").and_then(|v| v.as_str()), Some("value1"));
    assert_eq!(table.get("key2").and_then(|v| v.as_integer()), Some(42));
}

#[test]
fn test_config_state_build_table_from_flat_nested() {
    let mut state = ConfigState::new();
    state.insert("a.b.c".to_string(), toml::Value::String("deep".to_string()));
    state.insert("a.b.d".to_string(), toml::Value::Integer(1));

    let table = state.build_table_from_flat("a");
    let b = table.get("b").and_then(|v| v.as_table());
    assert!(b.is_some());
    assert_eq!(
        b.and_then(|t| t.get("c").and_then(|v| v.as_str())),
        Some("deep")
    );
    assert_eq!(
        b.and_then(|t| t.get("d").and_then(|v| v.as_integer())),
        Some(1)
    );
}

#[test]
fn test_parse_and_flatten_integration() {
    let fixture = TestFixture::new();
    let path = fixture.path("config.toml");
    fs::write(
        &path,
        r#"
[main]
name = "test"
count = 42
enabled = true

[nested]
key = "value"
"#,
    )
    .expect("write");

    let toml_value = parse_toml_file(path.to_str().unwrap()).expect("parse");
    let table = toml_value.as_table().expect("should be table");
    let flat = flatten_table("", table);

    assert_eq!(flat.get("main.name").and_then(|v| v.as_str()), Some("test"));
    assert_eq!(
        flat.get("main.count").and_then(|v| v.as_integer()),
        Some(42)
    );
    assert_eq!(
        flat.get("main.enabled").and_then(|v| v.as_bool()),
        Some(true)
    );
    assert_eq!(
        flat.get("nested.key").and_then(|v| v.as_str()),
        Some("value")
    );
}

#[test]
fn test_parse_toml_file_with_nested_tables() {
    let fixture = TestFixture::new();
    let path = fixture.path("config.toml");
    fs::write(
        &path,
        r#"
[database]
host = "localhost"
port = 5432

[database.credentials]
user = "admin"
password = "secret"
"#,
    )
    .expect("write");

    let toml_value = parse_toml_file(path.to_str().unwrap()).expect("parse");
    let table = toml_value.as_table().expect("should be table");

    assert_eq!(
        table
            .get("database")
            .and_then(|t| t.as_table())
            .and_then(|t| t.get("host"))
            .and_then(|v| v.as_str()),
        Some("localhost")
    );

    assert_eq!(
        table
            .get("database")
            .and_then(|t| t.as_table())
            .and_then(|t| t.get("credentials"))
            .and_then(|t| t.as_table())
            .and_then(|t| t.get("user"))
            .and_then(|v| v.as_str()),
        Some("admin")
    );
}

#[test]
fn test_parse_toml_file_duration_string() {
    let fixture = TestFixture::new();
    let path = fixture.path("config.toml");
    fs::write(
        &path,
        r#"
[settings]
timeout = "30s"
interval = "5m"
"#,
    )
    .expect("write");

    let toml_value = parse_toml_file(path.to_str().unwrap()).expect("parse");
    let table = toml_value.as_table().expect("should be table");

    assert_eq!(
        table
            .get("settings")
            .and_then(|t| t.as_table())
            .and_then(|t| t.get("timeout"))
            .and_then(|v| v.as_str()),
        Some("30s")
    );

    assert_eq!(
        table
            .get("settings")
            .and_then(|t| t.as_table())
            .and_then(|t| t.get("interval"))
            .and_then(|v| v.as_str()),
        Some("5m")
    );
}
