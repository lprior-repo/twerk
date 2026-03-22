//! Docker image reference parsing following functional-rust conventions.
//!
//! This module parses Docker image references like:
//! - `ubuntu:mantic`
//! - `localhost:9090/ubuntu:mantic`
//! - `my-registry/ubuntu:mantic-2.7`
//!
//! # Architecture
//!
//! - **Data**: `Reference` struct holds parsed components
//! - **Calc**: Pure parsing functions with regex
//! - **Actions**: I/O pushed to boundary (file loading, etc.)

use once_cell::sync::Lazy;
use regex::Regex;
use thiserror::Error;

/// Maximum total number of characters in a repository name.
const NAME_TOTAL_LENGTH_MAX: usize = 255;

/// Domain errors for reference parsing.
#[derive(Debug, Error)]
pub enum ReferenceError {
    #[error("invalid reference format")]
    InvalidFormat,

    #[error("repository name must be lowercase")]
    ContainsUppercase,

    #[error("repository name must have at least one component")]
    NameEmpty,

    #[error("repository name must not be more than {0} characters")]
    NameTooLong(usize),
}

/// Parsed Docker image reference.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Reference {
    /// Registry domain (e.g., "localhost:9090", "my-registry").
    pub domain: String,
    /// Image path without domain (e.g., "ubuntu").
    pub path: String,
    /// Image tag (e.g., "mantic").
    pub tag: String,
}

impl Reference {
    /// Returns the full image name including domain if present.
    #[must_use]
    pub fn full_name(&self) -> String {
        if self.domain.is_empty() {
            self.path.clone()
        } else {
            format!("{}/{}", self.domain, self.path)
        }
    }

    /// Returns the image name with tag, e.g., "ubuntu:mantic".
    #[must_use]
    pub fn with_tag(&self, tag: &str) -> Self {
        Self {
            domain: self.domain.clone(),
            path: self.path.clone(),
            tag: tag.to_string(),
        }
    }
}

// ----------------------------------------------------------------------------
// Regex patterns - compiled once at startup using once_cell::Lazy
// ----------------------------------------------------------------------------

/// Matches alphanumeric characters (lowercase only).
static ALPHA_NUMERIC_REGEX: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"[a-z0-9]+").expect("regex should be valid"));

/// Matches separators: period, underscore, double underscore, or dashes.
static SEPARATOR_REGEX: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"(?:[._]|__|[-]*)").expect("regex should be valid"));

/// Matches a domain component (alphanumeric with optional hyphens).
static DOMAIN_COMPONENT_REGEX: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r"(?:[a-zA-Z0-9]|[a-zA-Z0-9][a-zA-Z0-9-]*[a-zA-Z0-9])")
        .expect("regex should be valid")
});

/// Matches tag names per Docker spec.
static TAG_REGEX: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"[\w][\w.-]{0,127}").expect("regex should be valid"));

/// Matches digest values.
static DIGEST_REGEX: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r"[A-Za-z][A-Za-z0-9]*(?:[-_+.][A-Za-z][A-Za-z0-9]*)*[:][[:xdigit:]]{32,}")
        .expect("regex should be valid")
});

/// Matches name components.
static NAME_COMPONENT_REGEX: Lazy<Regex> = Lazy::new(|| {
    let base = ALPHA_NUMERIC_REGEX.to_string();
    let sep = SEPARATOR_REGEX.to_string();
    Regex::new(&format!("(?:{})(?:{})*", base, sep)).expect("regex should be valid")
});

/// Matches domain with optional port.
static DOMAIN_REGEX: Lazy<Regex> = Lazy::new(|| {
    let dc = DOMAIN_COMPONENT_REGEX.to_string();
    Regex::new(&format!("{}(?:\\.{})*(?::[0-9]+)?", dc, dc)).expect("regex should be valid")
});

/// Matches name part of reference (domain/name components).
static NAME_REGEX: Lazy<Regex> = Lazy::new(|| {
    let name_comp = NAME_COMPONENT_REGEX.to_string();
    let slash = r"\/";
    let dom = format!("(?:{})?{}", *DOMAIN_REGEX, slash);
    let path = format!("(?:{})(?:{}(?:{}))*", name_comp, slash, name_comp);
    Regex::new(&format!("{}{}", dom, path)).expect("regex should be valid")
});

/// Matches name with domain captured.
static ANCHORED_NAME_REGEX: Lazy<Regex> = Lazy::new(|| {
    let dom = DOMAIN_COMPONENT_REGEX.to_string();
    let slash = r"\/";
    let name_comp = NAME_COMPONENT_REGEX.to_string();
    let _path = format!("(?:{})(?:{}(?:{}))*", name_comp, slash, name_comp);
    let full = format!(
        "^(?:({}){})?({}(?:{}(?:{}))*)$",
        dom, slash, name_comp, slash, name_comp
    );
    Regex::new(&full).expect("regex should be valid")
});

/// Full reference regex with captures for name, tag, and digest.
static REFERENCE_REGEX: Lazy<Regex> = Lazy::new(|| {
    let name = NAME_REGEX.to_string();
    let tag = TAG_REGEX.to_string();
    let digest = DIGEST_REGEX.to_string();
    let colon = r":";
    let at = r"@";
    let full = format!(
        "^({})(?:{}:({}))?(?:{}@({}))?$",
        name, colon, tag, at, digest
    );
    Regex::new(&full).expect("regex should be valid")
});

// ----------------------------------------------------------------------------
// Parsing functions
// ----------------------------------------------------------------------------

/// Parses a string into a `Reference`.
///
/// # Errors
///
/// Returns `ReferenceError` if the string is not a valid reference format.
pub fn parse(s: &str) -> Result<Reference, ReferenceError> {
    if s.is_empty() {
        return Err(ReferenceError::NameEmpty);
    }

    let captures = REFERENCE_REGEX.captures(s).ok_or_else(|| {
        // Check if lowercase version would match (indicates uppercase)
        if REFERENCE_REGEX.is_match(&s.to_lowercase()) {
            ReferenceError::ContainsUppercase
        } else {
            ReferenceError::InvalidFormat
        }
    })?;

    let full_match = captures.get(1).map(|m| m.as_str()).unwrap_or("");
    if full_match.len() > NAME_TOTAL_LENGTH_MAX {
        return Err(ReferenceError::NameTooLong(full_match.len()));
    }

    let tag = captures
        .get(3)
        .map(|m| m.as_str().to_string())
        .unwrap_or_default();

    // Parse domain and path from the name
    let (domain, path) = parse_name(full_match)?;

    Ok(Reference { domain, path, tag })
}

/// Parses the name component (domain + path) from a reference.
fn parse_name(name: &str) -> Result<(String, String), ReferenceError> {
    if let Some(captures) = ANCHORED_NAME_REGEX.captures(name) {
        let domain = captures
            .get(1)
            .map(|m| m.as_str().to_string())
            .unwrap_or_default();
        let path = captures
            .get(2)
            .map(|m| m.as_str().to_string())
            .unwrap_or_else(|| name.to_string());
        Ok((domain, path))
    } else {
        Ok((String::new(), name.to_string()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_simple_name_with_tag() {
        let result = parse("ubuntu:mantic").expect("should parse");
        assert_eq!("", result.domain);
        assert_eq!("ubuntu", result.path);
        assert_eq!("mantic", result.tag);
    }

    #[test]
    fn test_parse_with_port() {
        let result = parse("localhost:9090/ubuntu:mantic").expect("should parse");
        assert_eq!("localhost:9090", result.domain);
        assert_eq!("ubuntu", result.path);
        assert_eq!("mantic", result.tag);
    }

    #[test]
    fn test_parse_with_port_and_tag_suffix() {
        let result = parse("localhost:9090/ubuntu:mantic-2.7").expect("should parse");
        assert_eq!("localhost:9090", result.domain);
        assert_eq!("ubuntu", result.path);
        assert_eq!("mantic-2.7", result.tag);
    }

    #[test]
    fn test_parse_with_custom_registry() {
        let result = parse("my-registry/ubuntu:mantic-2.7").expect("should parse");
        assert_eq!("my-registry", result.domain);
        assert_eq!("ubuntu", result.path);
        assert_eq!("mantic-2.7", result.tag);
    }

    #[test]
    fn test_parse_without_tag() {
        let result = parse("my-registry/ubuntu").expect("should parse");
        assert_eq!("my-registry", result.domain);
        assert_eq!("ubuntu", result.path);
        assert_eq!("", result.tag);
    }

    #[test]
    fn test_parse_short_name() {
        let result = parse("ubuntu").expect("should parse");
        assert_eq!("", result.domain);
        assert_eq!("ubuntu", result.path);
        assert_eq!("", result.tag);
    }

    #[test]
    fn test_parse_with_latest_tag() {
        let result = parse("ubuntu:latest").expect("should parse");
        assert_eq!("", result.domain);
        assert_eq!("ubuntu", result.path);
        assert_eq!("latest", result.tag);
    }

    #[test]
    fn test_parse_empty() {
        let result = parse("");
        assert!(result.is_err());
    }
}
