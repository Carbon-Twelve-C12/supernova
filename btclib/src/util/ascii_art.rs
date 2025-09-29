use crossterm::{
    cursor::MoveTo,
    execute,
    style::{Color, ResetColor, SetForegroundColor},
    terminal::{Clear, ClearType},
    ExecutableCommand,
};
use std::{
    io::{self, Write},
    thread::sleep,
    time::Duration,
};

const SUPERNOVA_ASCII: &str = r#"
   _____                       _   __                
  / ___/__  ______  ___  _____/ | / /___ _   ______ _
  \__ \/ / / / __ \/ _ \/ ___/  |/ / __ \ | / / __ `/
 ___/ / /_/ / /_/ /  __/ /  / /|  / /_/ / |/ / /_/ / 
/____/\__,_/ .___/\___/_/  /_/ |_/\____/|___/\__,_/  
          /_/                                         
"#;

const SUPERNOVA_LARGE: &str = r#"
███████╗██╗   ██╗██████╗ ███████╗██████╗ ███╗   ██╗ ██████╗ ██╗   ██╗ █████╗ 
██╔════╝██║   ██║██╔══██╗██╔════╝██╔══██╗████╗  ██║██╔═══██╗██║   ██║██╔══██╗
███████╗██║   ██║██████╔╝█████╗  ██████╔╝██╔██╗ ██║██║   ██║██║   ██║███████║
╚════██║██║   ██║██╔═══╝ ██╔══╝  ██╔══██╗██║╚██╗██║██║   ██║╚██╗ ██╔╝██╔══██║
███████║╚██████╔╝██║     ███████╗██║  ██║██║ ╚████║╚██████╔╝ ╚████╔╝ ██║  ██║
╚══════╝ ╚═════╝ ╚═╝     ╚══════╝╚═╝  ╚═╝╚═╝  ╚═══╝ ╚═════╝   ╚═══╝  ╚═╝  ╚═╝
"#;

/// Clear terminal screen
pub fn clear_screen() -> io::Result<()> {
    let mut stdout = io::stdout();
    stdout.execute(Clear(ClearType::All))?;
    stdout.execute(MoveTo(0, 0))?;
    Ok(())
}

/// Display the supernova logo without animation
pub fn display_logo() -> io::Result<()> {
    let mut stdout = io::stdout();

    execute!(stdout, Clear(ClearType::All))?;
    execute!(stdout, MoveTo(0, 0))?;

    let logo = [
        r"   _____                       _   __                    ",
        r"  / ___/__  ______  ___  ____/ | / /___ _   ______ _    ",
        r"  \__ \/ / / / __ \/ _ \/ ___/  |/ / __ \ | / / __ `/    ",
        r" ___/ / /_/ / /_/ /  __/ /  / /|  / /_/ / |/ / /_/ /     ",
        r"/____/\__,_/ .___/\___/_/  /_/ |_/\____/|___/\__,_/      ",
        r"          /_/                                            ",
    ];

    execute!(stdout, SetForegroundColor(Color::Cyan))?;
    for line in &logo {
        println!("{}", line);
    }
    execute!(stdout, ResetColor)?;

    println!();
    println!(" Next-Generation Blockchain with Enhanced Security, ");
    println!(" Scalability, and Environmental Awareness");
    println!();

    Ok(())
}

/// Display supernova logo with a slide-in animation from left to right
pub fn animate_logo_slide_in() -> io::Result<()> {
    let mut stdout = io::stdout();
    clear_screen()?;

    // Get the lines from the ASCII art
    let lines: Vec<&str> = SUPERNOVA_LARGE.lines().collect();
    let max_width = lines.iter().map(|line| line.len()).max().unwrap_or(0);

    // Slide in from left to right
    for x in 0..=max_width {
        clear_screen()?;

        for (i, line) in lines.iter().enumerate() {
            if !line.is_empty() {
                let visible = if x < line.len() { &line[0..x] } else { line };
                stdout.execute(MoveTo(0, i as u16))?;
                stdout.execute(SetForegroundColor(Color::Magenta))?;
                print!("{}", visible);
                stdout.execute(ResetColor)?;
            }
        }

        stdout.flush()?;
        sleep(Duration::from_millis(10));
    }

    // Pause at the end to let the user see the complete logo
    sleep(Duration::from_millis(500));
    Ok(())
}

/// Display supernova logo with a dissolve animation from bottom to top
pub fn animate_logo_dissolve_out() -> io::Result<()> {
    let mut stdout = io::stdout();

    // Get the lines from the ASCII art
    let lines: Vec<&str> = SUPERNOVA_LARGE.lines().collect();
    let line_count = lines.len();

    // Dissolve out from bottom to top
    for dissolve_line in (0..line_count).rev() {
        clear_screen()?;

        for (i, line) in lines.iter().enumerate() {
            if i < dissolve_line && !line.is_empty() {
                stdout.execute(MoveTo(0, i as u16))?;
                stdout.execute(SetForegroundColor(Color::Magenta))?;
                print!("{}", line);
                stdout.execute(ResetColor)?;
            }
        }

        stdout.flush()?;
        sleep(Duration::from_millis(100));
    }

    clear_screen()?;
    Ok(())
}

/// Perform the complete logo animation - slide in from left and dissolve from bottom
pub fn animate_logo_complete() -> io::Result<()> {
    animate_logo_slide_in()?;
    sleep(Duration::from_millis(1000)); // Display complete logo for 1 second
    animate_logo_dissolve_out()?;
    Ok(())
}

/// Display testnet startup animation with progress information
pub fn testnet_startup_animation() -> io::Result<()> {
    let mut stdout = io::stdout();
    clear_screen()?;

    // Display the logo first
    animate_logo_slide_in()?;

    // Display testnet info below the logo
    let logo_height = SUPERNOVA_LARGE.lines().count();
    let progress_steps = [
        "Initializing supernova Testnet...",
        "Loading network configuration...",
        "Starting blockchain services...",
        "Initializing P2P connections...",
        "Setting up Lightning Network...",
        "Starting environmental tracking...",
        "Activating quantum-resistant signatures...",
        "Testnet ready!",
    ];

    stdout.execute(MoveTo(0, (logo_height + 2) as u16))?;
    stdout.execute(SetForegroundColor(Color::Green))?;
    println!("supernova Testnet Launch Sequence");
    stdout.execute(ResetColor)?;

    for (i, step) in progress_steps.iter().enumerate() {
        stdout.execute(MoveTo(2, (logo_height + 4 + i) as u16))?;
        print!("[ ] {}", step);
        stdout.flush()?;
        sleep(Duration::from_millis(500));

        stdout.execute(MoveTo(2, (logo_height + 4 + i) as u16))?;
        stdout.execute(SetForegroundColor(Color::Green))?;
        print!("[✓]");
        stdout.execute(ResetColor)?;
        stdout.flush()?;
    }

    // Final pause to see the completed animation
    sleep(Duration::from_millis(1000));

    Ok(())
}

/// Display a spinning loader with a message
pub fn display_spinner(message: &str, duration_ms: u64) -> io::Result<()> {
    let spinner_chars = ['⠋', '⠙', '⠹', '⠸', '⠼', '⠴', '⠦', '⠧', '⠇', '⠏'];
    let iterations = duration_ms / 100;
    let mut stdout = io::stdout();

    for i in 0..iterations {
        let idx = (i % spinner_chars.len() as u64) as usize;

        execute!(stdout, MoveTo(0, i as u16))?;
        execute!(stdout, SetForegroundColor(Color::Magenta))?;
        print!("{} {}", spinner_chars[idx], message);
        execute!(stdout, ResetColor)?;

        stdout.flush()?;
        sleep(Duration::from_millis(100));
        print!("\r                                                  \r");
    }

    Ok(())
}

/// Display a progress bar
pub fn display_progress_bar(message: &str, progress: f64, width: usize) -> io::Result<()> {
    let mut stdout = io::stdout();
    let progress = progress.clamp(0.0, 1.0);
    let filled_width = (progress * width as f64) as usize;
    let empty_width = width - filled_width;

    for i in 0..3 {
        execute!(stdout, MoveTo(0, i as u16))?;
        execute!(stdout, SetForegroundColor(Color::Magenta))?;

        if i == 0 {
            print!("{} [{:.1}%]", message, progress * 100.0);
        } else if i == 1 {
            print!("[{}{}]", "█".repeat(filled_width), " ".repeat(empty_width));
        }

        execute!(stdout, ResetColor)?;
    }

    stdout.flush()?;
    Ok(())
}

/// Display the startup sequence
pub fn display_startup_sequence() -> io::Result<()> {
    let mut stdout = io::stdout();

    display_logo()?;

    let logo_height = 8;

    execute!(stdout, MoveTo(0, (logo_height + 2) as u16))?;
    execute!(stdout, SetForegroundColor(Color::Green))?;
    println!("Starting supernova Blockchain...");
    execute!(stdout, ResetColor)?;

    for i in 0..5 {
        execute!(stdout, MoveTo(2, (logo_height + 4 + i) as u16))?;
        display_spinner(&format!("Initializing component {}/5...", i + 1), 500)?;
    }

    for i in 0..5 {
        execute!(stdout, MoveTo(2, (logo_height + 4 + i) as u16))?;
        execute!(stdout, SetForegroundColor(Color::Green))?;
        println!("✓ Component {}/5 initialized", i + 1);
        execute!(stdout, ResetColor)?;
    }

    Ok(())
}
