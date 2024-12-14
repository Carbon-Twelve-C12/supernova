use std::io;
use ratatui::{
    backend::CrosstermBackend,
    widgets::{Block, Borders, List, ListItem, Paragraph},
    layout::{Layout, Constraint, Direction},
    Terminal,
};
use crossterm::{
    event::{self, Event, KeyCode},
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
    ExecutableCommand,
};
use crate::core::Wallet;

pub struct WalletTui {
    wallet: Wallet,
    terminal: Terminal<CrosstermBackend<io::Stdout>>,
}

impl WalletTui {
    pub fn new(wallet: Wallet) -> Result<Self, io::Error> {
        // Setup terminal
        enable_raw_mode()?;
        let mut stdout = io::stdout();
        stdout.execute(EnterAlternateScreen)?;
        let backend = CrosstermBackend::new(stdout);
        let terminal = Terminal::new(backend)?;

        Ok(Self { wallet, terminal })
    }

    pub fn run(&mut self) -> Result<(), io::Error> {
        loop {
            // Draw UI
            self.terminal.draw(|frame| {
                let chunks = Layout::default()
                    .direction(Direction::Vertical)
                    .margin(1)
                    .constraints([
                        Constraint::Length(3),  // Address
                        Constraint::Length(3),  // Balance
                        Constraint::Min(0),     // UTXOs
                    ].as_ref())
                    .split(frame.size());

                // Wallet Address
                let address = Paragraph::new(format!("Address: {}", self.wallet.get_address()))
                    .block(Block::default().title("Wallet").borders(Borders::ALL));
                frame.render_widget(address, chunks[0]);

                // Balance
                let balance = Paragraph::new(format!("Balance: {} NOVA", self.wallet.get_balance()))
                    .block(Block::default().title("Balance").borders(Borders::ALL));
                frame.render_widget(balance, chunks[1]);

                // UTXO List
                let utxos: Vec<ListItem> = self.wallet.utxos.values()
                    .flatten()
                    .map(|utxo| {
                        ListItem::new(format!(
                            "UTXO: {} - Amount: {} NOVA",
                            hex::encode(&utxo.tx_hash[..8]),
                            utxo.amount
                        ))
                    })
                    .collect();

                let utxo_list = List::new(utxos)
                    .block(Block::default().title("UTXOs").borders(Borders::ALL));
                frame.render_widget(utxo_list, chunks[2]);
            })?;

            // Handle input
            if event::poll(std::time::Duration::from_millis(100))? {
                if let Event::Key(key) = event::read()? {
                    match key.code {
                        KeyCode::Char('q') => break,
                        _ => {}
                    }
                }
            }
        }

        Ok(())
    }
}

impl Drop for WalletTui {
    fn drop(&mut self) {
        // Cleanup terminal
        let _ = disable_raw_mode();
        let _ = self.terminal.backend_mut().execute(LeaveAlternateScreen);
    }
}