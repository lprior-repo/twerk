#![no_main]

use libfuzzer_sys::fuzz_target;
use twerk_infrastructure::runtime::docker::parse;

fuzz_target!(|data: &[u8]| {
    let s = String::from_utf8_lossy(data);
    let _ = parse(&s);
});
