#![no_main]

use std::collections::HashMap;

use libfuzzer_sys::fuzz_target;
use twerk_core::redact::redact_vars;

#[derive(Debug, arbitrary::Arbitrary)]
struct Input {
    keys: Vec<String>,
    values: Vec<String>,
    secret_keys: Vec<String>,
    secret_values: Vec<String>,
}

fuzz_target!(|input: Input| {
    let m: HashMap<String, String> = input
        .keys
        .into_iter()
        .zip(input.values.into_iter())
        .collect();
    let secrets: HashMap<String, String> = input
        .secret_keys
        .into_iter()
        .zip(input.secret_values.into_iter())
        .collect();

    let redacted = redact_vars(&m, &secrets);

    // The result must have the same keys as the input map
    assert_eq!(m.keys().collect::<Vec<_>>(), redacted.keys().collect::<Vec<_>>());
});
