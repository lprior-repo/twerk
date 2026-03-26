//! Pure helper functions for Docker runtime.

use std::time::Duration;
use tar::Archive as TarArchive;

pub fn parse_go_duration(input: &str) -> Result<Duration, String> {
    let mut total = Duration::ZERO;
    let mut current = String::new();
    for c in input.chars() {
        if c.is_ascii_digit() || c == '.' {
            current.push(c);
        } else {
            let num: f64 = current.parse()
                .map_err(|e| format!("invalid duration '{}': {}", current, e))?;
            total += match c {
                'h' => Duration::from_secs_f64(num * 3600.0),
                'm' => Duration::from_secs_f64(num * 60.0),
                's' => Duration::from_secs_f64(num),
                _ => return Err(format!("unknown unit: {}", c)),
            };
            current.clear();
        }
    }
    if !current.is_empty() {
        return Err(format!("trailing: {}", current));
    }
    Ok(total)
}

pub fn parse_memory_bytes(input: &str) -> Result<i64, String> {
    let input = input.trim();
    let (num_str, multiplier) =
        if let Some(s) = input.strip_suffix("TB").or_else(|| input.strip_suffix("tb")) {
            (s.trim(), 1_099_511_627_776_i64)
        } else if let Some(s) = input.strip_suffix("GB").or_else(|| input.strip_suffix("gb")) {
            (s.trim(), 1_073_741_824_i64)
        } else if let Some(s) = input.strip_suffix("MB").or_else(|| input.strip_suffix("mb")) {
            (s.trim(), 1_048_576_i64)
        } else if let Some(s) = input.strip_suffix("KB").or_else(|| input.strip_suffix("kb")) {
            (s.trim(), 1024_i64)
        } else if let Some(s) = input.strip_suffix("B").or_else(|| input.strip_suffix("b")) {
            (s.trim(), 1_i64)
        } else {
            return input.parse::<i64>()
                .map_err(|e| format!("cannot parse '{}': {}", input, e));
        };

    let num = num_str.parse::<f64>()
        .map_err(|e| format!("cannot parse '{}': {}", num_str, e))?;
    Ok((num * multiplier as f64) as i64)
}

pub fn parse_tar_contents(tar_bytes: &[u8]) -> String {
    let mut archive = TarArchive::new(tar_bytes);
    let Ok(entries) = archive.entries() else {
        return String::new();
    };
    for entry in entries {
        let Ok(mut entry) = entry else {
            continue;
        };
        let mut buf = Vec::new();
        if std::io::Read::read_to_end(&mut entry, &mut buf).is_ok() {
            if let Ok(s) = String::from_utf8(buf) {
                return s;
            }
        }
    }
    String::new()
}

#[must_use]
pub fn slugify(input: &str) -> String {
    input.chars()
        .map(|c| if c.is_alphanumeric() { c.to_ascii_lowercase() } else { '-' })
        .collect::<String>()
        .split('-')
        .filter(|s| !s.is_empty())
        .collect::<Vec<_>>()
        .join("-")
}
