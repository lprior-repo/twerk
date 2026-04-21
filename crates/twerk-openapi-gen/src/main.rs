use std::path::{Path, PathBuf};
use std::process::ExitCode;

fn workspace_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .to_path_buf()
}

fn main() -> ExitCode {
    match twerk_web::api::openapi::sync_tracked_artifacts(&workspace_root()) {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("{error:#}");
            ExitCode::FAILURE
        }
    }
}
