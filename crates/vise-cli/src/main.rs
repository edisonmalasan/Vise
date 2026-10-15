use std::process::ExitCode;

mod cli;
mod ops;
mod preset;

fn main() -> ExitCode {
    let cli = cli::parse();
    match cli::run(cli) {
        Ok(()) => ExitCode::SUCCESS,
        Err((code, message)) => {
            eprintln!("{message}");
            ExitCode::from(code as u8)
        }
    }
}
