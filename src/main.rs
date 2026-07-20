mod cli;
mod commands;
mod model;
mod repository;
mod storage;
mod update;

/// Application version: injected from `TSLAY_VERSION` at build time (Makefile / CI),
/// falling back to `CARGO_PKG_VERSION` for plain `cargo build`.
pub const VERSION: &str = match option_env!("TSLAY_VERSION") {
    Some(v) => v,
    None => env!("CARGO_PKG_VERSION"),
};

fn main() -> std::process::ExitCode {
    cli::run()
}
