use btclib::util::ascii_art;
use std::io;

fn main() -> io::Result<()> {
    println!("supernova ASCII Art");
    println!("========================\n");

    println!("1. Static Logo:");
    ascii_art::display_logo()?;

    println!("\nPress Enter to see the animated slide-in logo...");
    let mut buffer = String::new();
    io::stdin().read_line(&mut buffer)?;

    ascii_art::animate_logo_slide_in()?;

    println!("\nPress Enter to see the animated dissolve-out effect...");
    buffer.clear();
    io::stdin().read_line(&mut buffer)?;

    ascii_art::animate_logo_dissolve_out()?;

    println!("\nPress Enter to see the complete animation (slide in + dissolve out)...");
    buffer.clear();
    io::stdin().read_line(&mut buffer)?;

    ascii_art::animate_logo_complete()?;

    println!("\nPress Enter to see the testnet startup animation...");
    buffer.clear();
    io::stdin().read_line(&mut buffer)?;

    ascii_art::testnet_startup_animation()?;

    println!("\nAnimation demo complete!");
    Ok(())
}
