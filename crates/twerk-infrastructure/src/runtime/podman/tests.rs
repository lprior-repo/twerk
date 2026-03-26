//! Podman runtime tests — matching Go's podman_test.go 1:1.
//!
//! # Go Test Mapping
//!
//! | Go Test                               | Rust Test                                |
//! |---------------------------------------|------------------------------------------|
//! | TestPodmanRunTaskCMD                   | test_podman_run_task_cmd                  |
//! | TestPodmanRunTaskRun                   | test_podman_run_task_run                  |
//! | TestPodmanCustomEntrypoint             | test_podman_custom_entrypoint             |
//! | TestPodmanRunPrePost                   | test_podman_run_pre_post                  |
//! | TestPodmanRunTaskWithVolume            | test_podman_run_task_with_volume          |
//! | TestPodmanRunTaskWithVolumeAndCustomWorkdir | test_podman_run_volume_custom_workdir  |
//! | TestPodmanRunTaskWithVolumeAndWorkdir  | test_podman_run_volume_and_workdir        |
//! | TestPodmanRunTaskInitWorkdir           | test_podman_run_task_init_workdir         |
//! | TestPodmanRunTaskInitWorkdirLs         | test_podman_run_task_init_workdir_ls      |
//! | TestPodmanRunTaskWithTimeout           | test_podman_run_task_with_timeout         |
//! | TestPodmanRunTaskWithError             | test_podman_run_task_with_error           |
//! | TestPodmanHealthCheck                  | test_podman_health_check                  |
//! | TestPodmanHealthCheckFailed            | test_podman_health_check_failed           |
//! | TestRunTaskWithPrivilegedModeOn        | test_run_task_privileged_on               |
//! | TestRunTaskWithPrivilegedModeOff       | test_run_task_privileged_off              |
//!
//! All tests run automatically. Requires podman to be installed and running.

// Re-export test modules
pub mod validation;
pub mod lifecycle;
pub mod execution;
pub mod integration;
