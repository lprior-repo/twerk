use std::process::ExitCode;

fn main() -> ExitCode {
    match twerk_web::api::openapi::generate_json() {
        Ok(json) => {
            println!("{json}");
            ExitCode::SUCCESS
        }
        Err(error) => {
            eprintln!("{error:#}");
            ExitCode::FAILURE
        }
    }
}
