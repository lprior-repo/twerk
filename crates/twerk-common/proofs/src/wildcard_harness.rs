use twerk_common::wildcard::{is_wild_pattern, wildcard_match};

#[kani::proof]
fn wildcard_match_star_matches_everything() {
    // Build a small bounded string via kani::any for each byte.
    let len: usize = kani::any();
    kani::assume(len <= 4);
    let mut buf = [0u8; 4];
    for i in 0..len {
        buf[i] = kani::any();
        // Keep bytes in printable ASCII range for well-formedness
        kani::assume(buf[i] >= 0x20 && buf[i] <= 0x7E);
    }
    let s = std::str::from_utf8(&buf[..len]).unwrap();
    assert!(wildcard_match("*", s), "star pattern matches any string");
}

#[kani::proof]
fn wildcard_match_empty_pattern_matches_only_empty() {
    assert!(wildcard_match("", ""), "empty pattern matches empty string");
    assert!(!wildcard_match("", "a"), "empty pattern does not match non-empty string");
}

#[kani::proof]
fn wildcard_match_no_star_exact_match() {
    assert!(wildcard_match("abc", "abc"), "exact pattern matches identical string");
    assert!(!wildcard_match("abc", "abd"), "exact pattern does not match different string");
}

#[kani::proof]
fn is_wild_pattern_with_star() {
    assert!(is_wild_pattern("*"), "pattern containing star is wild");
}

#[kani::proof]
fn is_wild_pattern_without_star() {
    assert!(!is_wild_pattern("abc"), "pattern without star is not wild");
}
