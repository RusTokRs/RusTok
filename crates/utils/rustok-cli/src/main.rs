use std::process::ExitCode;

#[tokio::main]
async fn main() -> ExitCode {
    let exit = rustok_cli::run_with_environment(std::env::args()).await;
    if !exit.stdout.is_empty() {
        print!("{}", exit.stdout);
    }
    if !exit.stderr.is_empty() {
        eprint!("{}", exit.stderr);
    }

    ExitCode::from(exit.code as u8)
}
