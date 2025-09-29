use btclib::util::ascii_art;
use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(author, version, about = "supernova ASCII Art Banner Tool")]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Display static logo
    Static,

    /// Display slide-in animation
    SlideIn,

    /// Display dissolve-out animation
    DissolveOut,

    /// Display complete animation (slide in + dissolve out)
    Complete,

    /// Display testnet startup animation
    Testnet,
}

fn main() {
    let cli = Cli::parse();

    match cli.command {
        Commands::Static => {
            if let Err(e) = ascii_art::display_logo() {
                eprintln!("Error displaying logo: {}", e);
            }
        }
        Commands::SlideIn => {
            if let Err(e) = ascii_art::animate_logo_slide_in() {
                eprintln!("Error animating logo: {}", e);
            }
        }
        Commands::DissolveOut => {
            if let Err(e) = ascii_art::animate_logo_dissolve_out() {
                eprintln!("Error animating logo: {}", e);
            }
        }
        Commands::Complete => {
            if let Err(e) = ascii_art::animate_logo_complete() {
                eprintln!("Error animating logo: {}", e);
            }
        }
        Commands::Testnet => {
            if let Err(e) = ascii_art::testnet_startup_animation() {
                eprintln!("Error displaying testnet animation: {}", e);
            }
        }
    }
}
