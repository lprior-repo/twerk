//! Container execution coordinator

use super::errors::PodmanError;
use super::types::{PodmanRuntime, Task};

#[allow(dead_code)]
impl PodmanRuntime {
    /// Main execution entry point
    pub(crate) async fn do_run_inner(&self, task: &mut Task) -> Result<(), PodmanError> {
        // Setup work directory
        let (workdir, output_file, progress_file) = self.setup_workdir(task).await?;

        // Write task files
        Self::write_task_files(&workdir, &task.files).await?;

        // Add privileged flag if enabled
        if self.privileged {
            // This is handled in build_create_command
        }

        // Execute container
        self.execute_container(task, &workdir, &output_file, &progress_file).await
    }
}
