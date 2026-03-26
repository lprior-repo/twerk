// ----------------------------------------------------------------------------
// Image Tests
// ----------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    // Image-specific tests if any
    #[test]
    fn test_image_digest_format() {
        // Valid SHA256 digest format
        let digest = "sha256:0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef";
        assert_eq!(digest.len(), 71); // sha256: + 64 hex chars
        assert!(digest.starts_with("sha256:"));
    }
}
