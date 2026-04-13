//! Banner display functionality
//!
//! Displays the Twerk ASCII art banner with version information.

use tracing::info;

/// The Twerk ASCII art banner
const BANNER: &str = "
 _______  _  _  _  _______  ______    ___   _ 
|       || || || ||       ||    _ |  |   | | |
|_     _|| || || ||    ___||   | ||  |   |_| |
  |   |  | || || ||   |___ |   |_||_ |      _|
  |   |  | || || ||    ___||    __  ||     |_ 
  |   |  |       ||   |___ |   |  | ||    _  |
  |___|  |_______||_______||___|  |_||___| |_|
";

/// Banner display mode
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum BannerMode {
    /// Print to console (stdout)
    #[default]
    Console,
    /// Log using tracing (info level)
    Log,
    /// Don't display banner
    Off,
}

impl BannerMode {
    /// Parse banner mode from string
    pub fn from_str(s: &str) -> Self {
        match s.to_lowercase().as_str() {
            "off" => Self::Off,
            "log" => Self::Log,
            _ => Self::Console,
        }
    }
}

/// Display the Twerk banner with version information
///
/// # Arguments
///
/// * `mode` - How to display the banner
/// * `version` - The version string
/// * `git_commit` - The git commit hash
pub fn display_banner(mode: BannerMode, version: &str, git_commit: &str) {
    match mode {
        BannerMode::Off => {}
        BannerMode::Console => {
            println!("{BANNER}");
            println!(" {version} ({git_commit})");
        }
        BannerMode::Log => {
            info!("{}\n {} ({})", BANNER.trim(), version, git_commit);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn banner_mode_from_str_returns_expected_variant_for_supported_and_unknown_values() {
        assert_eq!(BannerMode::from_str("off"), BannerMode::Off);
        assert_eq!(BannerMode::from_str("console"), BannerMode::Console);
        assert_eq!(BannerMode::from_str("log"), BannerMode::Log);
        assert_eq!(BannerMode::from_str("unknown"), BannerMode::Console);
        assert_eq!(BannerMode::from_str("OFF"), BannerMode::Off);
        assert_eq!(BannerMode::from_str("Console"), BannerMode::Console);
    }

    #[test]
    fn banner_mode_from_str_is_case_insensitive_for_known_values() {
        assert_eq!(BannerMode::from_str("OFF"), BannerMode::Off);
        assert_eq!(BannerMode::from_str("Off"), BannerMode::Off);
        assert_eq!(BannerMode::from_str("LOG"), BannerMode::Log);
        assert_eq!(BannerMode::from_str("Log"), BannerMode::Log);
        assert_eq!(BannerMode::from_str("CONSOLE"), BannerMode::Console);
    }

    #[test]
    fn banner_mode_from_str_treats_whitespace_wrapped_values_as_console_default() {
        // from_str does NOT trim, so whitespace is preserved
        // " off " becomes " off " after to_lowercase(), which doesn't match "off"
        assert_eq!(BannerMode::from_str(" off "), BannerMode::Console);
        assert_eq!(BannerMode::from_str("log "), BannerMode::Console);
        assert_eq!(BannerMode::from_str(" log"), BannerMode::Console);
    }

    #[test]
    fn banner_mode_default_returns_console() {
        assert_eq!(BannerMode::default(), BannerMode::Console);
    }

    #[test]
    fn banner_constant_is_not_empty_and_contains_ascii_art_shape_markers() {
        assert!(!BANNER.is_empty());
        // Banner should contain ASCII art characters
        assert!(BANNER.contains('|'));
        assert!(BANNER.contains('_'));
    }

    #[test]
    fn banner_constant_contains_expected_branding_pattern() {
        // Banner should contain Twerk branding
        assert!(BANNER.contains("_______"));
    }

    #[test]
    fn banner_mode_equality_and_inequality_are_consistent() {
        assert_eq!(BannerMode::Off, BannerMode::Off);
        assert_eq!(BannerMode::Console, BannerMode::Console);
        assert_eq!(BannerMode::Log, BannerMode::Log);
        assert_ne!(BannerMode::Off, BannerMode::Console);
        assert_ne!(BannerMode::Console, BannerMode::Log);
    }

    #[test]
    fn banner_mode_copy_semantics_preserve_value() {
        let mode = BannerMode::Off;
        let _copied = mode;
        assert_eq!(mode, BannerMode::Off);
    }

    #[test]
    fn banner_mode_clone_semantics_preserve_value() {
        let mode = BannerMode::Log;
        let cloned = mode;
        assert_eq!(mode, cloned);
    }
}
