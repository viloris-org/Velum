mod architecture;
mod ci_metrics;
mod docs;
mod process;

use std::{env, path::PathBuf, process::ExitCode};

fn main() -> ExitCode {
    match run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("error: {error}");
            ExitCode::FAILURE
        }
    }
}

fn run() -> Result<(), String> {
    let root = workspace_root()?;
    let mut args = env::args().skip(1);
    match args.next().as_deref() {
        Some("architecture") => architecture::check(&root),
        Some("docs") => docs::check(&root),
        Some("test") => process::test_all(&root),
        Some("ci-metrics") => {
            let input = args
                .next()
                .ok_or("ci-metrics requires an input JSON path")?;
            let output = args
                .next()
                .ok_or("ci-metrics requires an output JSON path")?;
            if args.next().is_some() {
                return Err("ci-metrics accepts exactly two paths".into());
            }
            ci_metrics::evaluate_files(&root.join(input), &root.join(output))
        }
        Some(command) => Err(format!("unknown xtask command: {command}")),
        None => Err("usage: cargo xtask <architecture|docs|test|ci-metrics>".into()),
    }
}

fn workspace_root() -> Result<PathBuf, String> {
    let manifest = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    manifest
        .parent()
        .map(PathBuf::from)
        .ok_or_else(|| "xtask must be inside the workspace".into())
}
