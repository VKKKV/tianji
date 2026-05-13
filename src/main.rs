use std::env;
use std::process::ExitCode;

use tianji::{artifact_json, run_fixture_path, TianJiError};

fn main() -> ExitCode {
    match run(env::args().skip(1).collect()) {
        Ok(output) => {
            println!("{output}");
            ExitCode::SUCCESS
        }
        Err(error) => {
            eprintln!("error: {error}");
            ExitCode::from(1)
        }
    }
}

fn run(args: Vec<String>) -> Result<String, TianJiError> {
    let fixture_path = parse_run_fixture_args(&args)?;
    let artifact = run_fixture_path(fixture_path)?;
    artifact_json(&artifact)
}

fn parse_run_fixture_args(args: &[String]) -> Result<&str, TianJiError> {
    match args {
        [command, flag, fixture_path] if command == "run" && flag == "--fixture" => {
            Ok(fixture_path)
        }
        _ => Err(TianJiError::Usage(
            "usage: cargo run -- run --fixture <path>".to_string(),
        )),
    }
}
