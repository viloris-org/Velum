use std::process::ExitCode;

#[tokio::main]
async fn main() -> ExitCode {
    match velum_node::cli::run(std::env::args().skip(1)).await {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("error: {error}");
            ExitCode::FAILURE
        }
    }
}
