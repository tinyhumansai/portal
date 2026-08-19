//! The `portal` command-line entry point.
//!
//! Everything the binary does lives in the library's [`portal::cli`] module, so
//! the same commands can be driven from a test or embedded in another program.

/// Run the CLI, reporting a failure on stderr with a non-zero exit status.
#[tokio::main]
async fn main() -> std::process::ExitCode {
    match portal::cli::run().await {
        Ok(()) => std::process::ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("portal: {error}");
            std::process::ExitCode::FAILURE
        }
    }
}
