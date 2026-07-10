use std::{path::Path, process::Command};

pub fn test_all(root: &Path) -> Result<(), String> {
    super::architecture::check(root)?;
    super::docs::check(root)?;

    run(root, "node", &["experiments/stage0/validate.mjs"])?;
    run(
        root,
        "node",
        &["experiments/stage0/harness/harness.test.mjs"],
    )?;
    run(
        root,
        "node",
        &["experiments/stage0/results/validator.test.mjs"],
    )?;
    run(root, "node", &["experiments/stage0/results/validate.mjs"])?;
    run(root, "cargo", &["fmt", "--all", "--check"])?;
    run(
        root,
        "cargo",
        &[
            "clippy",
            "--workspace",
            "--all-targets",
            "--",
            "-D",
            "warnings",
        ],
    )?;
    run(root, "cargo", &["test", "--workspace", "--all-targets"])?;
    run(root, "cargo", &["deny", "check"])?;
    println!("All current Foundation checks passed.");
    Ok(())
}

fn run(root: &Path, program: &str, args: &[&str]) -> Result<(), String> {
    println!("running: {program} {}", args.join(" "));
    let status = Command::new(program)
        .args(args)
        .current_dir(root)
        .status()
        .map_err(|error| {
            if error.kind() == std::io::ErrorKind::NotFound {
                format!("required command {program:?} is not installed")
            } else {
                format!("cannot run {program}: {error}")
            }
        })?;
    if status.success() {
        Ok(())
    } else {
        Err(format!("{program} {} failed with {status}", args.join(" ")))
    }
}
