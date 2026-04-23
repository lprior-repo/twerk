#![no_main]

use arbitrary::{Arbitrary, Unstructured};
use libfuzzer_sys::fuzz_target;
use std::collections::HashMap;

/// Generates an arbitrary TOML-compatible value for fuzzing.
#[derive(Debug)]
enum FuzzTomlValue {
    String(String),
    Integer(i64),
    Float(f64),
    Boolean(bool),
    Array(Vec<FuzzTomlValue>),
    Table(HashMap<String, FuzzTomlValue>),
}

impl<'a> Arbitrary<'a> for FuzzTomlValue {
    fn arbitrary(u: &mut Unstructured<'a>) -> arbitrary::Result<Self> {
        let tag: u8 = u.int_in_range(0..=5)?;
        match tag {
            0 => Ok(FuzzTomlValue::String(String::arbitrary(u)?)),
            1 => Ok(FuzzTomlValue::Integer(i64::arbitrary(u)?)),
            2 => Ok(FuzzTomlValue::Float(f64::arbitrary(u)?)),
            3 => Ok(FuzzTomlValue::Boolean(bool::arbitrary(u)?)),
            4 => {
                let len = u.int_in_range(0..=4)?;
                let mut arr = Vec::with_capacity(len);
                for _ in 0..len {
                    arr.push(FuzzTomlValue::arbitrary(u)?);
                }
                Ok(FuzzTomlValue::Array(arr))
            }
            _ => {
                let len = u.int_in_range(0..=4)?;
                let mut table = HashMap::new();
                for _ in 0..len {
                    let key_len = u.int_in_range(1..=8)?;
                    let key_bytes = u.bytes(key_len)?;
                    let key: String = key_bytes
                        .iter()
                        .map(|&b| (b % 26 + b'a') as char)
                        .collect();
                    table.insert(key, FuzzTomlValue::arbitrary(u)?);
                }
                Ok(FuzzTomlValue::Table(table))
            }
        }
    }
}

fn to_toml_value(val: &FuzzTomlValue) -> toml::Value {
    match val {
        FuzzTomlValue::String(s) => toml::Value::String(s.clone()),
        FuzzTomlValue::Integer(i) => toml::Value::Integer(*i),
        FuzzTomlValue::Float(f) => toml::Value::Float(*f),
        FuzzTomlValue::Boolean(b) => toml::Value::Boolean(*b),
        FuzzTomlValue::Array(arr) => {
            toml::Value::Array(arr.iter().map(to_toml_value).collect())
        }
        FuzzTomlValue::Table(map) => {
            let table: toml::value::Table = map.iter().map(|(k, v)| (k.clone(), to_toml_value(v))).collect();
            toml::Value::Table(table)
        }
    }
}

fn extract_table(val: &FuzzTomlValue) -> Option<toml::value::Table> {
    match val {
        FuzzTomlValue::Table(map) => {
            let table: toml::value::Table = map.iter().map(|(k, v)| (k.clone(), to_toml_value(v))).collect();
            Some(table)
        }
        _ => None,
    }
}

fuzz_target!(|data: &[u8]| {
    let u = Unstructured::new(data);
    if let Ok(val) = FuzzTomlValue::arbitrary_take_rest(u) {
        if let Some(table) = extract_table(&val) {
            let _ = twerk_common::conf::parsing::flatten_table("", &table);
        }
    }
});
