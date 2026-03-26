#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn test_bool_env_override() {
        setup();
        env::set_var("TWERK_MAIN_ENABLED", "true");
        load_config().ok();
        assert!(bool("main.enabled")); // This should fail if my suspicion is correct
        cleanup();
    }
}
