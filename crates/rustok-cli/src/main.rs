use std::process::ExitCode;

fn main() -> ExitCode {
    let exit = rustok_cli::run_with_args(std::env::args());
    if !exit.stdout.is_empty() {
        print!("{}", exit.stdout);
    }
    if !exit.stderr.is_empty() {
        eprint!("{}", exit.stderr);
    }

    ExitCode::from(exit.code as u8)
}
