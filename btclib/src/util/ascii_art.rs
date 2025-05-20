use std::{thread::sleep, time::Duration, io::{self, Write}};
use crossterm::{
    cursor::{self, MoveTo},
    terminal::{Clear, ClearType},
    ExecutableCommand,
    style::{Color, SetForegroundColor, ResetColor},
    queue
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

/// Display the SuperNova logo without animation
pub fn display_logo() -> io::Result<()> {
    let mut stdout = io::stdout();
    stdout.execute(SetForegroundColor(Color::Cyan))?;
    println!("{}", SUPERNOVA_ASCII);
    stdout.execute(ResetColor)?;
    Ok(())
}

/// Display SuperNova logo with a slide-in animation from left to right
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

/// Display SuperNova logo with a dissolve animation from bottom to top
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
    let progress_steps = vec![
        "Initializing SuperNova Testnet...",
        "Loading network configuration...",
        "Starting blockchain services...", 
        "Initializing P2P connections...",
        "Setting up Lightning Network...",
        "Starting environmental tracking...",
        "Activating quantum-resistant signatures...",
        "Testnet ready!"
    ];
    
    stdout.execute(MoveTo(0, (logo_height + 2) as u16))?;
    stdout.execute(SetForegroundColor(Color::Green))?;
    println!("SuperNova Testnet Launch Sequence");
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