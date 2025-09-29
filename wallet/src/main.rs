mod cli;
mod core;
mod hdwallet;
mod history;
mod ui;

fn main() {
    env_logger::init();

    if let Err(e) = cli::run_cli() {
        eprintln!("Error: {}", e);
        std::process::exit(1);
    }
}
