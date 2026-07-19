mod cli;
mod commands;
mod model;
mod repository;
mod storage;

fn main() -> std::process::ExitCode {
    cli::run()
}
