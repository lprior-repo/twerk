//! Pagination domain types.
//!
//! # Design Principles
//!
//! - **Parse, don't validate**: Raw input is parsed into domain types at
//!   boundary entry points. Core logic receives only validated types.
//! - **Make illegal states unrepresentable**: Newtypes encode business rules
//!   that cannot be violated after construction.

use serde::{Deserialize, Serialize};
use thiserror::Error;

/// A validated page number (1-indexed).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(transparent)]
pub struct Page(u64);

#[derive(Debug, Clone, PartialEq, Error)]
pub enum PageError {
    #[error("page number must be at least 1")]
    TooSmall,
}

impl Page {
    /// Create a new `Page` from a 64-bit unsigned integer.
    ///
    /// # Errors
    ///
    /// Returns [`PageError`] if the page number is less than 1.
    pub fn new(page: u64) -> Result<Self, PageError> {
        if page < 1 {
            return Err(PageError::TooSmall);
        }
        Ok(Self(page))
    }

    /// Returns the page number as a u64.
    #[must_use]
    pub fn get(self) -> u64 {
        self.0
    }

    /// Returns the page number as an i64 (for API compatibility).
    #[must_use]
    pub fn as_i64(self) -> i64 {
        self.0.cast_signed()
    }
}

impl Default for Page {
    fn default() -> Self {
        Self(1)
    }
}

impl From<Page> for i64 {
    fn from(p: Page) -> Self {
        p.as_i64()
    }
}

impl TryFrom<i64> for Page {
    type Error = PageError;

    fn try_from(v: i64) -> Result<Self, Self::Error> {
        if v < 1 {
            return Err(PageError::TooSmall);
        }
        Ok(Self(v.cast_unsigned()))
    }
}

/// A validated page size.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(transparent)]
pub struct PageSize(u64);

#[derive(Debug, Clone, PartialEq, Error)]
pub enum PageSizeError {
    #[error("page size must be at least 1")]
    TooSmall,
    #[error("page size {size} exceeds maximum allowed ({max})")]
    TooLarge { size: u64, max: u64 },
}

impl PageSize {
    /// Maximum allowed page size.
    pub const MAX_VALUE: u64 = 100;

    /// Default page size.
    pub const DEFAULT: u64 = 10;

    /// Create a new `PageSize` from a 64-bit unsigned integer.
    ///
    /// # Errors
    ///
    /// Returns [`PageSizeError`] if the size is invalid.
    pub fn new(size: u64) -> Result<Self, PageSizeError> {
        if size < 1 {
            return Err(PageSizeError::TooSmall);
        }
        if size > Self::MAX_VALUE {
            return Err(PageSizeError::TooLarge {
                size,
                max: Self::MAX_VALUE,
            });
        }
        Ok(Self(size))
    }

    /// Create a `PageSize` with a default value if None.
    #[must_use]
    pub fn or_default(size: Option<u64>) -> Self {
        size.and_then(|s| Self::new(s).ok())
            .map_or_else(Self::default, |v| v.clone())
    }

    /// Returns the page size as a u64.
    #[must_use]
    pub fn get(self) -> u64 {
        self.0
    }

    /// Returns the page size as an i64 (for API compatibility).
    #[must_use]
    pub fn as_i64(self) -> i64 {
        self.0.cast_signed()
    }
}

impl Default for PageSize {
    fn default() -> Self {
        Self(Self::DEFAULT)
    }
}

impl From<PageSize> for i64 {
    fn from(p: PageSize) -> Self {
        p.as_i64()
    }
}

impl TryFrom<i64> for PageSize {
    type Error = PageSizeError;

    fn try_from(v: i64) -> Result<Self, Self::Error> {
        if v < 1 {
            return Err(PageSizeError::TooSmall);
        }
        let size = v.cast_unsigned();
        if size > Self::MAX_VALUE {
            return Err(PageSizeError::TooLarge {
                size,
                max: Self::MAX_VALUE,
            });
        }
        Ok(Self(size))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn page_valid() {
        assert!(matches!(Page::new(1), Ok(page) if page.get() == 1));
    }

    #[test]
    fn page_rejects_zero() {
        assert!(matches!(Page::new(0), Err(PageError::TooSmall)));
    }

    #[test]
    fn page_size_valid() {
        assert!(matches!(PageSize::new(20), Ok(size) if size.get() == 20));
    }

    #[test]
    fn page_size_clamped_to_max() {
        let size = PageSize::new(200);
        assert!(size.is_err());
    }
}
