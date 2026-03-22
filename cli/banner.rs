//! Banner display functionality
//!
//! Displays the Tork ASCII art banner with version information.

use tracing::info;

/// The Tork ASCII art banner
const BANNER: &str = "
 _______  _______  ______    ___   _ 
|       ||       ||    _ |  |   | | |
|_     _||   _   ||   | ||  |   |_| |
  |   |  |  | |  ||   |_||_ |      _|
  |   |  |  |_|  ||    __  ||     |_ 
  |   |  |       ||   |  | ||    _  |
  |___|  |_______||___|  |_||___| |_|
";

/// Banner display mode
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[derive(Default)]
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

/// Display the Tork banner with version information
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
            info!(
                "{}\n {} ({})",
                BANNER.trim(),
                version,
                git_commit
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_banner_mode_from_str() {
        assert_eq!(BannerMode::from_str("off"), BannerMode::Off);
        assert_eq!(BannerMode::from_str("console"), BannerMode::Console);
        assert_eq!(BannerMode::from_str("log"), BannerMode::Log);
        assert_eq!(BannerMode::from_str("unknown"), BannerMode::Console);
        assert_eq!(BannerMode::from_str("OFF"), BannerMode::Off);
        assert_eq!(BannerMode::from_str("Console"), BannerMode::Console);
    }

    #[test]
    fn test_banner_mode_default() {
        assert_eq!(BannerMode::default(), BannerMode::Console);
    }

    #[test]
    fn test_banner_not_empty() {
        assert!(!BANNER.is_empty());
        // Banner should contain ASCII art characters
        assert!(BANNER.contains('|'));
        assert!(BANNER.contains('_'));
    }
}
