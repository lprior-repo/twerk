#![no_main]

use libfuzzer_sys::fuzz_target;

/// Fuzz the duration-parsing logic from twerk_common::conf::lookup.
///
/// NOTE: `parse_duration` is a private function inside `conf::lookup` and
/// cannot be called from external crates. We replicate the same parsing
/// algorithm here so it can be fuzzed. If `parse_duration` is ever made
/// public, this target should be updated to call it directly.
fn parse_duration(s: &str) -> Option<()> {
    let s = s.trim();
    if s.is_empty() {
        return None;
    }
    parse_single_duration(s).or_else(|| parse_complex_duration(s))
}

fn parse_single_duration_with_value(val: &str, unit: &str) -> Option<()> {
    let val = val.trim();
    match unit {
        "ns" | "us" | "ms" | "m" | "h" | "d" => {
            val.parse::<i64>().ok()?;
        }
        "s" => {
            val.parse::<f64>().ok()?;
        }
        _ => return None,
    };
    Some(())
}

fn parse_single_duration(s: &str) -> Option<()> {
    if s.ends_with("ns") {
        parse_single_duration_with_value(s.trim_end_matches("ns"), "ns")
    } else if s.ends_with("us") || s.ends_with("\u{b5}s") {
        let val = if s.ends_with("us") {
            s.trim_end_matches("us")
        } else {
            s.trim_end_matches("\u{b5}s")
        };
        parse_single_duration_with_value(val, "us")
    } else if s.ends_with("ms") {
        parse_single_duration_with_value(s.trim_end_matches("ms"), "ms")
    } else if s.ends_with('s') {
        parse_single_duration_with_value(s.trim_end_matches('s'), "s")
    } else if s.ends_with('m') {
        parse_single_duration_with_value(s.trim_end_matches('m'), "m")
    } else if s.ends_with('h') {
        parse_single_duration_with_value(s.trim_end_matches('h'), "h")
    } else if s.ends_with('d') {
        parse_single_duration_with_value(s.trim_end_matches('d'), "d")
    } else {
        None
    }
}

fn parse_complex_duration(s: &str) -> Option<()> {
    let chars: Vec<char> = s.chars().collect();
    let mut current_num = String::new();
    let mut current_unit = String::new();
    let mut found_part = false;
    let mut i = 0;

    while i < chars.len() {
        let c = chars[i];
        if c.is_ascii_digit() || c == '.' || c == '-' {
            if !current_unit.is_empty() {
                parse_single_duration_with_value(&current_num, &current_unit)?;
                current_num.clear();
                current_unit.clear();
                found_part = true;
            }
            current_num.push(c);
        } else if c.is_alphabetic() {
            current_unit.push(c);
        }
        i += 1;
    }

    if !current_num.is_empty() && !current_unit.is_empty() {
        parse_single_duration_with_value(&current_num, &current_unit)?;
        found_part = true;
    }

    found_part.then_some(())
}

fuzz_target!(|data: &[u8]| {
    if let Ok(s) = std::str::from_utf8(data) {
        let _ = parse_duration(s);
    }
});
