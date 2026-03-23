//! Reexec command builder for privilege dropping

use tokio::process::Command;

/// Function type for reexec-style command execution
/// Takes arguments and returns a configured Command
pub type ReexecCommand = Box<dyn Fn(&[String]) -> Command + Send + Sync>;

/// Create a reexec command using std::process::Command
#[allow(dead_code)]
pub fn reexec_from_std(args: &[String]) -> Command {
    let mut cmd = Command::new(&args[0]);
    cmd.args(&args[1..]);
    cmd
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_reexec_from_std_single_arg() {
        let args = vec!["/bin/echo".to_string()];
        let cmd = reexec_from_std(&args);
        let program = cmd.as_std().get_program().to_string_lossy().into_owned();
        assert_eq!(program, "/bin/echo");
        assert_eq!(cmd.as_std().get_args().count(), 0);
    }

    #[test]
    fn test_reexec_from_std_multiple_args() {
        let args = vec![
            "/bin/echo".to_string(),
            "hello".to_string(),
            "world".to_string(),
        ];
        let cmd = reexec_from_std(&args);
        let program = cmd.as_std().get_program().to_string_lossy().into_owned();
        assert_eq!(program, "/bin/echo");
        let cmd_args: Vec<String> = cmd
            .as_std()
            .get_args()
            .map(|s| s.to_string_lossy().into_owned())
            .collect();
        assert_eq!(cmd_args, vec!["hello", "world"]);
    }
}
