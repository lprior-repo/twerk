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
