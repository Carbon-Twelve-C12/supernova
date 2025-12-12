mod cli;
mod backup_warning;
mod core;
mod hdwallet;
mod history;
mod password_strength;
mod ui;

fn main() {
    env_logger::init();

    if let Err(e) = cli::run_cli() {
        eprintln!("Error: {}", e);
        std::process::exit(1);
    }
}
