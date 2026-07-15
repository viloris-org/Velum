use std::{path::Path, process::Command};

pub fn test_all(root: &Path) -> Result<(), String> {
    super::architecture::check(root)?;
    super::docs::check(root)?;

    run(root, "node", &["validation/validate.mjs"])?;
    run(root, "node", &["validation/harness/harness.test.mjs"])?;
    run(root, "node", &["validation/results/validator.test.mjs"])?;
    run(root, "node", &["validation/results/validate.mjs"])?;
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

pub fn model_check(root: &Path) -> Result<(), String> {
    run(
        root,
        "cargo",
        &["test", "-p", "velum-session", "--all-targets"],
    )?;
    println!("Deterministic session tracer checks passed.");
    Ok(())
}

pub fn client_test(root: &Path) -> Result<(), String> {
    run(root, "cargo", &["build", "-p", "velum-client-ffi"])?;
    let library_name = if cfg!(target_os = "windows") {
        "velum_client_ffi.dll"
    } else if cfg!(target_os = "macos") {
        "libvelum_client_ffi.dylib"
    } else {
        "libvelum_client_ffi.so"
    };
    let library = root.join("target").join("debug").join(library_name);
    let library = library
        .to_str()
        .ok_or("native client library path is not UTF-8")?;
    let client = root.join("apps/velum-client/flutter");
    run(&client, "flutter", &["analyze"])?;
    println!("running: flutter test with native ABI library");
    let status = Command::new("flutter")
        .args([
            "test",
            &format!("--dart-define=VELUM_CLIENT_LIBRARY={library}"),
        ])
        .env("VELUM_CLIENT_LIBRARY", library)
        .current_dir(client)
        .status()
        .map_err(|error| format!("cannot run flutter test: {error}"))?;
    if !status.success() {
        return Err(format!("flutter test failed with {status}"));
    }
    println!("All client checks passed, including native ABI v2 and v3.");
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
