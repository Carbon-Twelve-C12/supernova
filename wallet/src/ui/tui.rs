use std::io;
use ratatui::{
    backend::CrosstermBackend,
    widgets::{Block, Borders, List, ListItem, Paragraph, Tabs, Table, Row, Cell},
    layout::{Layout, Constraint, Direction, Rect},
    style::{Style, Color, Modifier},
    text::{Span, Spans},
    Terminal,
};
use crossterm::{
    event::{self, Event, KeyCode, KeyModifiers},
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
    ExecutableCommand,
};
use crate::core::Wallet;
use crate::hdwallet::{HDWallet, HDAddress};
use crate::history::{TransactionHistory, TransactionRecord, TransactionDirection, TransactionStatus};

enum ActiveTab {
    Overview,
    Accounts,
    Transactions,
    UTXOs,
    Settings,
}

pub struct WalletTui {
    hd_wallet: HDWallet,
    transaction_history: TransactionHistory,
    terminal: Terminal<CrosstermBackend<io::Stdout>>,
    active_tab: ActiveTab,
    selected_account: u32,
}

impl WalletTui {
    pub fn new(hd_wallet: HDWallet, transaction_history: TransactionHistory) -> Result<Self, io::Error> {
        // Setup terminal
        enable_raw_mode()?;
        let mut stdout = io::stdout();
        stdout.execute(EnterAlternateScreen)?;
        let backend = CrosstermBackend::new(stdout);
        let terminal = Terminal::new(backend)?;

        Ok(Self { 
            hd_wallet, 
            transaction_history,
            terminal,
            active_tab: ActiveTab::Overview,
            selected_account: 0,
        })
    }

    pub fn run(&mut self) -> Result<(), io::Error> {
        loop {
            // Draw UI
            self.terminal.draw(|frame| {
                let size = frame.size();
                
                // Create top-level layout with tabs and content
                let chunks = Layout::default()
                    .direction(Direction::Vertical)
                    .margin(1)
                    .constraints([
                        Constraint::Length(3),  // Tabs
                        Constraint::Min(0),     // Content
                    ].as_ref())
                    .split(size);
                
                // Render tabs
                let tab_titles = vec!["Overview", "Accounts", "Transactions", "UTXOs", "Settings"];
                let tabs = Tabs::new(
                    tab_titles.iter().map(|t| Spans::from(Span::styled(*t, Style::default()))).collect()
                )
                .select(match self.active_tab {
                    ActiveTab::Overview => 0,
                    ActiveTab::Accounts => 1,
                    ActiveTab::Transactions => 2,
                    ActiveTab::UTXOs => 3,
                    ActiveTab::Settings => 4,
                })
                .block(Block::default().title("SuperNova Wallet").borders(Borders::ALL))
                .style(Style::default().fg(Color::White))
                .highlight_style(Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD));
                
                frame.render_widget(tabs, chunks[0]);
                
                // Render the content based on active tab
                match self.active_tab {
                    ActiveTab::Overview => {
                        self.render_overview(frame, chunks[1]);
                    },
                    ActiveTab::Accounts => {
                        self.render_accounts(frame, chunks[1]);
                    },
                    ActiveTab::Transactions => {
                        self.render_transactions(frame, chunks[1]);
                    },
                    ActiveTab::UTXOs => {
                        self.render_utxos(frame, chunks[1]);
                    },
                    ActiveTab::Settings => {
                        self.render_settings(frame, chunks[1]);
                    },
                }
            })?;

            // Handle input
            if event::poll(std::time::Duration::from_millis(100))? {
                if let Event::Key(key) = event::read()? {
                    match key.code {
                        KeyCode::Char('q') => {
                            // Quit
                            if key.modifiers.contains(KeyModifiers::CONTROL) {
                                break;
                            }
                        },
                        KeyCode::Char('1') | KeyCode::Char('o') => {
                            self.active_tab = ActiveTab::Overview;
                        },
                        KeyCode::Char('2') | KeyCode::Char('a') => {
                            self.active_tab = ActiveTab::Accounts;
                        },
                        KeyCode::Char('3') | KeyCode::Char('t') => {
                            self.active_tab = ActiveTab::Transactions;
                        },
                        KeyCode::Char('4') | KeyCode::Char('u') => {
                            self.active_tab = ActiveTab::UTXOs;
                        },
                        KeyCode::Char('5') | KeyCode::Char('s') => {
                            self.active_tab = ActiveTab::Settings;
                        },
                        KeyCode::Tab => {
                            // Cycle through tabs
                            self.active_tab = match self.active_tab {
                                ActiveTab::Overview => ActiveTab::Accounts,
                                ActiveTab::Accounts => ActiveTab::Transactions,
                                ActiveTab::Transactions => ActiveTab::UTXOs,
                                ActiveTab::UTXOs => ActiveTab::Settings,
                                ActiveTab::Settings => ActiveTab::Overview,
                            };
                        },
                        KeyCode::Up => {
                            // Handle up navigation in lists
                            if matches!(self.active_tab, ActiveTab::Accounts) && self.selected_account > 0 {
                                self.selected_account -= 1;
                            }
                        },
                        KeyCode::Down => {
                            // Handle down navigation in lists
                            if matches!(self.active_tab, ActiveTab::Accounts) {
                                let account_count = self.hd_wallet.list_accounts().len() as u32;
                                if self.selected_account < account_count - 1 {
                                    self.selected_account += 1;
                                }
                            }
                        },
                        _ => {}
                    }
                }
            }
        }

        Ok(())
    }
    
    fn render_overview(&self, frame: &mut ratatui::Frame<CrosstermBackend<io::Stdout>>, area: Rect) {
        // Split area into sections
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(3),  // Total balance
                Constraint::Length(3),  // Address count
                Constraint::Length(6),  // Transaction summary
                Constraint::Min(0),     // Recent transactions
            ].as_ref())
            .split(area);
        
        // Total balance across all accounts
        let total_balance = self.hd_wallet.get_total_balance();
        let balance_text = format!("Total Balance: {} NOVA", total_balance);
        let balance = Paragraph::new(balance_text)
            .block(Block::default().title("Balance").borders(Borders::ALL))
            .style(Style::default().fg(Color::Green));
        frame.render_widget(balance, chunks[0]);
        
        // Address count
        let address_count = self.hd_wallet.get_address_count();
        let address_text = format!("Total Addresses: {}", address_count);
        let addresses = Paragraph::new(address_text)
            .block(Block::default().title("Addresses").borders(Borders::ALL));
        frame.render_widget(addresses, chunks[1]);
        
        // Transaction summary
        let sent = self.transaction_history.get_total_sent();
        let received = self.transaction_history.get_total_received();
        let fees = self.transaction_history.get_total_fees();
        let net_flow = self.transaction_history.get_net_flow();
        
        let tx_summary = vec![
            format!("Total Sent: {} NOVA", sent),
            format!("Total Received: {} NOVA", received),
            format!("Total Fees: {} NOVA", fees),
            format!("Net Flow: {} NOVA", net_flow),
        ];
        
        let summary = Paragraph::new(tx_summary.join("\n"))
            .block(Block::default().title("Transaction Summary").borders(Borders::ALL));
        frame.render_widget(summary, chunks[2]);
        
        // Recent transactions
        let recent_txs = self.transaction_history.get_all_transactions();
        let tx_items: Vec<ListItem> = recent_txs.iter().take(10).map(|tx| {
            let direction_symbol = match tx.direction {
                TransactionDirection::Incoming => "+ ",
                TransactionDirection::Outgoing => "- ",
                TransactionDirection::SelfTransfer => "↔ ",
            };
            
            let status_str = match tx.status {
                TransactionStatus::Pending => "[Pending]",
                TransactionStatus::Confirmed(conf) => format!("[Confirmed: {}]", conf),
                TransactionStatus::Failed => "[Failed]",
            };
            
            let label = tx.label.as_deref().unwrap_or("");
            
            ListItem::new(format!(
                "{}{} NOVA - {} - {}",
                direction_symbol, tx.amount, status_str, label
            ))
        }).collect();
        
        let transactions = List::new(tx_items)
            .block(Block::default().title("Recent Transactions").borders(Borders::ALL));
        frame.render_widget(transactions, chunks[3]);
    }
    
    fn render_accounts(&self, frame: &mut ratatui::Frame<CrosstermBackend<io::Stdout>>, area: Rect) {
        // Split area into two panes
        let chunks = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([
                Constraint::Percentage(30),  // Account list
                Constraint::Percentage(70),  // Account details
            ].as_ref())
            .split(area);
        
        // Render account list
        let accounts = self.hd_wallet.list_accounts();
        let account_items: Vec<ListItem> = accounts.iter().map(|account| {
            let style = if account.index == self.selected_account {
                Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)
            } else {
                Style::default()
            };
            
            ListItem::new(format!("{}: {}", account.index, account.name))
                .style(style)
        }).collect();
        
        let account_list = List::new(account_items)
            .block(Block::default().title("Accounts").borders(Borders::ALL));
        frame.render_widget(account_list, chunks[0]);
        
        // Render selected account details
        if let Some(account) = self.hd_wallet.get_account(self.selected_account) {
            let balance = self.hd_wallet.get_account_balance(self.selected_account);
            
            let receiving_addresses = account.get_receiving_addresses();
            let change_addresses = account.get_change_addresses();
            
            // Split account details area
            let detail_chunks = Layout::default()
                .direction(Direction::Vertical)
                .constraints([
                    Constraint::Length(3),  // Account info
                    Constraint::Percentage(50),  // Receiving addresses
                    Constraint::Percentage(50),  // Change addresses
                ].as_ref())
                .split(chunks[1]);
            
            // Account info
            let account_info = Paragraph::new(format!(
                "Name: {}\nBalance: {} NOVA",
                account.name, balance
            ))
            .block(Block::default().title(format!("Account #{}", account.index)).borders(Borders::ALL));
            frame.render_widget(account_info, detail_chunks[0]);
            
            // Receiving addresses
            let receiving_items: Vec<ListItem> = receiving_addresses.iter().map(|addr| {
                ListItem::new(format!("{}: {}", addr.index, addr.address))
            }).collect();
            
            let receiving_list = List::new(receiving_items)
                .block(Block::default().title("Receiving Addresses").borders(Borders::ALL));
            frame.render_widget(receiving_list, detail_chunks[1]);
            
            // Change addresses
            let change_items: Vec<ListItem> = change_addresses.iter().map(|addr| {
                ListItem::new(format!("{}: {}", addr.index, addr.address))
            }).collect();
            
            let change_list = List::new(change_items)
                .block(Block::default().title("Change Addresses").borders(Borders::ALL));
            frame.render_widget(change_list, detail_chunks[2]);
        } else {
            // No account selected or account doesn't exist
            let message = Paragraph::new("No account selected")
                .block(Block::default().title("Account Details").borders(Borders::ALL));
            frame.render_widget(message, chunks[1]);
        }
    }
    
    fn render_transactions(&self, frame: &mut ratatui::Frame<CrosstermBackend<io::Stdout>>, area: Rect) {
        let transactions = self.transaction_history.get_all_transactions();
        
        if transactions.is_empty() {
            let message = Paragraph::new("No transactions found")
                .block(Block::default().title("Transactions").borders(Borders::ALL));
            frame.render_widget(message, area);
            return;
        }
        
        // Create rows for transaction table
        let rows: Vec<Row> = transactions.iter().map(|tx| {
            let direction_symbol = match tx.direction {
                TransactionDirection::Incoming => "↓",
                TransactionDirection::Outgoing => "↑",
                TransactionDirection::SelfTransfer => "↔",
            };
            
            let status_str = match tx.status {
                TransactionStatus::Pending => "Pending",
                TransactionStatus::Confirmed(conf) => &format!("Confirmed ({})", conf),
                TransactionStatus::Failed => "Failed",
            };
            
            let tx_hash_short = &tx.tx_hash[0..8];
            let timestamp = chrono::DateTime::from_timestamp(tx.timestamp as i64, 0)
                .map(|dt| dt.format("%Y-%m-%d %H:%M").to_string())
                .unwrap_or_else(|| "Unknown".to_string());
            
            Row::new(vec![
                Cell::from(tx_hash_short),
                Cell::from(timestamp),
                Cell::from(direction_symbol),
                Cell::from(tx.amount.to_string()),
                Cell::from(tx.fee.to_string()),
                Cell::from(status_str),
                Cell::from(tx.label.as_deref().unwrap_or("")),
            ])
        }).collect();
        
        // Create the table
        let header_cells = ["Tx Hash", "Date", "Type", "Amount", "Fee", "Status", "Label"]
            .iter()
            .map(|h| Cell::from(*h).style(Style::default().fg(Color::Yellow)));
        let header = Row::new(header_cells).style(Style::default().add_modifier(Modifier::BOLD));
        
        let table = Table::new(rows)
            .header(header)
            .block(Block::default().title("Transactions").borders(Borders::ALL))
            .widths(&[
                Constraint::Length(10),  // Tx hash
                Constraint::Length(18),  // Date
                Constraint::Length(4),   // Type
                Constraint::Length(10),  // Amount
                Constraint::Length(8),   // Fee
                Constraint::Length(15),  // Status
                Constraint::Min(10),     // Label
            ]);
            
        frame.render_widget(table, area);
    }
    
    fn render_utxos(&self, frame: &mut ratatui::Frame<CrosstermBackend<io::Stdout>>, area: Rect) {
        // Get all UTXOs for the selected account
        let utxos = self.hd_wallet.get_account_utxos(self.selected_account);
        
        if utxos.is_empty() {
            let message = Paragraph::new(format!(
                "No UTXOs found for account #{}", self.selected_account
            ))
            .block(Block::default().title("UTXOs").borders(Borders::ALL));
            frame.render_widget(message, area);
            return;
        }
        
        // Create rows for UTXO table
        let rows: Vec<Row> = utxos.iter().map(|(addr, utxo)| {
            let tx_hash_short = hex::encode(&utxo.tx_hash[0..4]);
            
            Row::new(vec![
                Cell::from(tx_hash_short),
                Cell::from(utxo.output_index.to_string()),
                Cell::from(utxo.amount.to_string()),
                Cell::from(addr),
            ])
        }).collect();
        
        // Create the table
        let header_cells = ["Tx Hash", "Index", "Amount", "Address"]
            .iter()
            .map(|h| Cell::from(*h).style(Style::default().fg(Color::Yellow)));
        let header = Row::new(header_cells).style(Style::default().add_modifier(Modifier::BOLD));
        
        let table = Table::new(rows)
            .header(header)
            .block(Block::default().title(format!("UTXOs (Account #{})", self.selected_account)).borders(Borders::ALL))
            .widths(&[
                Constraint::Length(10),  // Tx hash
                Constraint::Length(5),   // Index
                Constraint::Length(10),  // Amount
                Constraint::Min(20),     // Address
            ]);
            
        frame.render_widget(table, area);
    }
    
    fn render_settings(&self, frame: &mut ratatui::Frame<CrosstermBackend<io::Stdout>>, area: Rect) {
        let information = vec![
            "Keyboard Shortcuts:",
            "1 or O - Overview tab",
            "2 or A - Accounts tab",
            "3 or T - Transactions tab",
            "4 or U - UTXOs tab",
            "5 or S - Settings tab",
            "Tab - Cycle through tabs",
            "↑/↓ - Navigate lists",
            "Ctrl+Q - Quit",
            "",
            "Wallet Information:",
            &format!("Active account: {}", self.selected_account),
            &format!("Address count: {}", self.hd_wallet.get_address_count()),
            &format!("Account count: {}", self.hd_wallet.list_accounts().len()),
        ];
        
        let settings = Paragraph::new(information.join("\n"))
            .block(Block::default().title("Settings").borders(Borders::ALL));
        frame.render_widget(settings, area);
    }
}

impl Drop for WalletTui {
    fn drop(&mut self) {
        // Cleanup terminal
        let _ = disable_raw_mode();
        let _ = self.terminal.backend_mut().execute(LeaveAlternateScreen);
    }
}