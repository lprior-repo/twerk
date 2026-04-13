#![no_main]

use libfuzzer_sys::fuzz_target;

fuzz_target!(|_data: &[u8]| {
    panic!("RED: fuzz target for TriggerId parser pending implementation");
});
