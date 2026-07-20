mod cli;
mod commands;
mod model;
mod repository;
mod storage;
mod update;

fn main() -> std::process::ExitCode {
    cli::run()
}
