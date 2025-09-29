use std::io;
use ratatui::{
    backend::CrosstermBackend,
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, List, ListItem, Paragraph, Tabs, ListState},
    Frame, Terminal,
};
use crossterm::{
    event::{self, DisableMouseCapture, EnableMouseCapture, Event, KeyCode, KeyEvent, KeyModifiers},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};

use crate::{
    hdwallet::{HDWallet, AccountType, HDAddress},
    history::{TransactionHistory, TransactionDirection, TransactionStatus},
};
use btclib::storage::utxo_set::UtxoSet;

#[derive(Debug)]
pub enum InputMode {
    Normal,
    AccountCreation,
    TransactionLabeling,
    AddressDisplay,
}

#[derive(Debug)]
pub enum Message {
    Info(String),
    Success(String),
    Error(String),
}

pub struct WalletTui {
    wallet: HDWallet,
    history: TransactionHistory,
    current_tab: Tab,
    accounts_state: ListState,
    transactions_state: ListState,
    input_mode: InputMode,
    input_text: String,
    message: Option<Message>,
    last_generated_address: Option<HDAddress>,
    selected_transaction: Option<String>, // Transaction hash
    utxo_set: UtxoSet, // Add UTXO set
}

#[derive(PartialEq, Clone, Copy)]
enum Tab {
    Overview,
    Accounts,
    Transactions,
    Help,
}

impl WalletTui {
    pub fn new(wallet: HDWallet, history: TransactionHistory) -> Result<Self, io::Error> {
        let mut accounts_state = ListState::default();
        accounts_state.select(Some(0)); // Select first account by default
        
        Ok(Self {
            wallet,
            history,
            current_tab: Tab::Overview,
            accounts_state,
            transactions_state: ListState::default(),
            input_mode: InputMode::Normal,
            input_text: String::new(),
            message: None,
            last_generated_address: None,
            selected_transaction: None,
            utxo_set: UtxoSet::new_in_memory(1000), // Create in-memory UTXO set
        })
    }

    pub fn run(&mut self) -> Result<(), io::Error> {
        enable_raw_mode()?;
        let mut stdout = io::stdout();
        execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;
        let backend = CrosstermBackend::new(stdout);
        let mut terminal = Terminal::new(backend)?;

        let res = self.run_app(&mut terminal);

        disable_raw_mode()?;
        execute!(
            terminal.backend_mut(),
            LeaveAlternateScreen,
            DisableMouseCapture
        )?;
        terminal.show_cursor()?;

        res
    }

    fn run_app<B: ratatui::backend::Backend>(&mut self, terminal: &mut Terminal<B>) -> Result<(), io::Error> {
        loop {
            terminal.draw(|f| self.render(f))?;

            if let Event::Key(key) = event::read()? {
                match self.input_mode {
                    InputMode::Normal => self.handle_normal_mode(key)?,
                    InputMode::AccountCreation => self.handle_account_creation_mode(key)?,
                    InputMode::TransactionLabeling => self.handle_transaction_labeling_mode(key)?,
                    InputMode::AddressDisplay => self.handle_address_display_mode(key)?,
                }
            }
        }
    }

    fn render(&mut self, f: &mut Frame) {
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .margin(1)
            .constraints([
                Constraint::Length(3),  // Tabs
                Constraint::Min(1),     // Content
                Constraint::Length(3),  // Status bar/message
            ].as_ref())
            .split(f.size());

        // Render tabs
        let titles = ["Overview", "Accounts", "Transactions", "Help"];
        let tabs = Tabs::new(titles.iter().map(|t| {
                    let (first, rest) = t.split_at(1);
                    Line::from(vec![
                        Span::styled(first, Style::default().fg(Color::Yellow)),
                        Span::styled(rest, Style::default().fg(Color::White)),
                    ])
        }).collect::<Vec<_>>())
            .block(Block::default().borders(Borders::ALL).title("supernova Wallet"))
            .select(self.current_tab as usize)
                .style(Style::default().fg(Color::White))
            .highlight_style(Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD));

                f.render_widget(tabs, chunks[0]);

        // Render main content based on current tab
                match self.current_tab {
                    Tab::Overview => self.render_overview(f, chunks[1]),
                    Tab::Accounts => self.render_accounts(f, chunks[1]),
                    Tab::Transactions => self.render_transactions(f, chunks[1]),
            Tab::Help => self.render_help(f, chunks[1]),
        }

        // Render input prompt if in edit mode
        match self.input_mode {
            InputMode::Normal => self.render_status_bar(f, chunks[2]),
            InputMode::AccountCreation => self.render_input_prompt(f, chunks[2], "Enter account name: "),
            InputMode::TransactionLabeling => self.render_input_prompt(f, chunks[2], "Enter transaction label: "),
            InputMode::AddressDisplay => self.render_address_display(f, chunks[2]),
        }
    }

    fn render_status_bar(&self, f: &mut Frame, area: Rect) {
        let status_text = match &self.message {
            Some(Message::Info(msg)) => Line::from(vec![
                Span::styled("INFO: ", Style::default().fg(Color::Blue).add_modifier(Modifier::BOLD)),
                Span::raw(msg)
            ]),
            Some(Message::Success(msg)) => Line::from(vec![
                Span::styled("SUCCESS: ", Style::default().fg(Color::Green).add_modifier(Modifier::BOLD)),
                Span::raw(msg)
            ]),
            Some(Message::Error(msg)) => Line::from(vec![
                Span::styled("ERROR: ", Style::default().fg(Color::Red).add_modifier(Modifier::BOLD)),
                Span::raw(msg)
            ]),
            None => {
                let help_text = match self.current_tab {
                    Tab::Overview => "Press ? for help",
                    Tab::Accounts => "Press n to create new account | a to generate address | ? for help",
                    Tab::Transactions => "Press l to label transaction | ? for help",
                    Tab::Help => "Press q to quit help | arrows to navigate",
                };
                Line::from(help_text)
            }
        };

        let status_bar = Paragraph::new(status_text)
            .block(Block::default().borders(Borders::ALL).title("Status"));
        f.render_widget(status_bar, area);
    }

    fn render_input_prompt(&self, f: &mut Frame, area: Rect, prompt: &str) {
        let input = Paragraph::new(Line::from(vec![
            Span::raw(prompt),
            Span::styled(&self.input_text, Style::default().fg(Color::Yellow))
        ]))
        .block(Block::default().borders(Borders::ALL).title("Input"));
        f.render_widget(input, area);
        
        // Show cursor at current input position
        f.set_cursor(
            area.x + prompt.len() as u16 + self.input_text.len() as u16 + 1,
            area.y + 1,
        );
    }

    fn render_address_display(&self, f: &mut Frame, area: Rect) {
        let address_text = if let Some(address) = &self.last_generated_address {
            Line::from(vec![
                Span::styled("New address: ", Style::default().fg(Color::Green)),
                Span::styled(address.get_address(), Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD))
            ])
        } else {
            Line::from("No address generated")
        };

        let address_display = Paragraph::new(address_text)
            .block(Block::default().borders(Borders::ALL).title("Address (Press Enter to continue)"));
        f.render_widget(address_display, area);
    }

    fn render_overview(&self, f: &mut Frame, area: Rect) {
        let total_balance = self.wallet.get_total_balance(&self.utxo_set).unwrap_or(0);
        let total_sent = self.history.get_total_sent();
        let total_received = self.history.get_total_received();
        let net_flow = self.history.get_net_flow();
        let account_count = self.wallet.list_accounts().len();
        let address_count = self.wallet.get_address_count();
        let transaction_count = self.history.get_all_transactions().len();

        let text = vec![
            Line::from(vec![
                Span::raw("Total Balance: "),
                Span::styled(format!("{} sats", total_balance), Style::default().fg(Color::Green).add_modifier(Modifier::BOLD)),
            ]),
            Line::from(Span::raw("")),
            Line::from(vec![
                Span::raw("Total Sent: "),
                Span::styled(format!("{} sats", total_sent), Style::default().fg(Color::Red)),
            ]),
            Line::from(vec![
                Span::raw("Total Received: "),
                Span::styled(format!("{} sats", total_received), Style::default().fg(Color::Green)),
            ]),
            Line::from(vec![
                Span::raw("Net Flow: "),
                Span::styled(
                    format!("{} sats", net_flow),
                    Style::default().fg(if net_flow >= 0 { Color::Green } else { Color::Red }),
                ),
            ]),
            Line::from(Span::raw("")),
            Line::from(vec![
                Span::raw("Accounts: "),
                Span::styled(format!("{}", account_count), Style::default().fg(Color::Yellow)),
            ]),
            Line::from(vec![
                Span::raw("Addresses: "),
                Span::styled(format!("{}", address_count), Style::default().fg(Color::Yellow)),
            ]),
            Line::from(vec![
                Span::raw("Transactions: "),
                Span::styled(format!("{}", transaction_count), Style::default().fg(Color::Yellow)),
            ]),
            Line::from(Span::raw("")),
            Line::from(Span::styled(
                "Press Tab to navigate between tabs",
                Style::default().fg(Color::Blue),
            )),
        ];

        let overview = Paragraph::new(text)
            .block(Block::default().borders(Borders::ALL).title("Overview"));

        f.render_widget(overview, area);
    }

    fn render_accounts(&mut self, f: &mut Frame, area: Rect) {
        // Collect account data first to avoid borrowing conflicts
        let accounts_data: Vec<_> = {
            let accounts = self.wallet.list_accounts();
            accounts.iter().map(|(index, account)| {
                let balance = self.wallet.get_balance(&account.name, &self.utxo_set).unwrap_or(0);
                let addr_count = account.addresses.len();
                (*index, account.name.clone(), account.account_type, balance, addr_count)
            }).collect()
        };
        
        let items: Vec<ListItem> = accounts_data
            .iter()
            .map(|(index, name, account_type, balance, addr_count)| {
                ListItem::new(vec![
                    Line::from(vec![
                        Span::styled(format!("{}. ", index), Style::default().fg(Color::DarkGray)),
                        Span::styled(name, Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)),
                        Span::raw(" - "),
                        Span::styled(format!("{} sats", balance), Style::default().fg(Color::Green)),
                    ]),
                    Line::from(vec![
                        Span::raw("   "),
                        Span::styled(format!("Type: {:?}", account_type), Style::default().fg(Color::Blue)),
                        Span::raw(" | "),
                        Span::styled(format!("Addresses: {}", addr_count), Style::default().fg(Color::Blue)),
                    ]),
                ])
            })
            .collect();

        let accounts_list = List::new(items)
            .block(Block::default().borders(Borders::ALL).title("Accounts"))
            .highlight_style(Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD))
            .highlight_symbol(">> ");

        let account_count = accounts_data.len();
        if account_count > 0 && self.accounts_state.selected().is_none() {
            self.accounts_state.select(Some(0));
        }

        f.render_stateful_widget(accounts_list, area, &mut self.accounts_state);
    }

    fn render_transactions(&mut self, f: &mut Frame, area: Rect) {
        let transactions = self.history.get_all_transactions();
        let items: Vec<ListItem> = transactions
            .iter()
            .map(|tx| {
            let amount_color = match tx.direction {
                TransactionDirection::Sent => Color::Red,
                TransactionDirection::Received => Color::Green,
            };

                let status_color = match &tx.status {
                TransactionStatus::Pending => Color::Yellow,
                TransactionStatus::Confirmed(_) => Color::Green,
                TransactionStatus::Failed => Color::Red,
            };

                let status_text = match &tx.status {
                    TransactionStatus::Pending => "Pending".to_string(),
                    TransactionStatus::Confirmed(n) => format!("Confirmed ({})", n),
                    TransactionStatus::Failed => "Failed".to_string(),
                };

                let label_text = if let Some(label) = &tx.label {
                    format!(" - {}", label)
                } else {
                    String::new()
                };

                ListItem::new(vec![
                    Line::from(vec![
                Span::styled(
                    tx.timestamp.format("%Y-%m-%d %H:%M").to_string(),
                    Style::default().fg(Color::Blue),
                ),
                Span::raw(" - "),
                Span::styled(
                    format!("{} sats", tx.amount),
                            Style::default().fg(amount_color).add_modifier(Modifier::BOLD),
                        ),
                        Span::styled(
                            label_text,
                            Style::default().fg(Color::Yellow),
                        ),
                    ]),
                    Line::from(vec![
                        Span::raw("   "),
                Span::styled(
                            status_text,
                    Style::default().fg(status_color),
                ),
                        Span::raw(" | "),
                        Span::styled(
                            format!("Tx: {}...", &tx.hash[0..8]),
                            Style::default().fg(Color::DarkGray),
                        ),
                    ]),
                ])
            })
            .collect();

        let transactions_list = List::new(items)
            .block(Block::default().borders(Borders::ALL).title("Transactions"))
            .highlight_style(Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD))
            .highlight_symbol(">> ");

        let tx_count = transactions.len();
        if tx_count > 0 && self.transactions_state.selected().is_none() {
            self.transactions_state.select(Some(0));
        }

        f.render_stateful_widget(transactions_list, area, &mut self.transactions_state);
    }

    fn render_help(&self, f: &mut Frame, area: Rect) {
        let text = vec![
            Line::from(vec![
                Span::styled("supernova Wallet Help", Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD))
            ]),
            Line::from(""),
            Line::from(vec![
                Span::styled("Global Shortcuts:", Style::default().add_modifier(Modifier::BOLD))
            ]),
            Line::from("  Tab       - Cycle through tabs"),
            Line::from("  q         - Quit application"),
            Line::from("  ?         - Show/hide help"),
            Line::from("  Esc       - Cancel current operation"),
            Line::from(""),
            Line::from(vec![
                Span::styled("Accounts Tab:", Style::default().add_modifier(Modifier::BOLD))
            ]),
            Line::from("  ↑/↓       - Navigate accounts"),
            Line::from("  n         - Create new account"),
            Line::from("  a         - Generate new address for selected account"),
            Line::from(""),
            Line::from(vec![
                Span::styled("Transactions Tab:", Style::default().add_modifier(Modifier::BOLD))
            ]),
            Line::from("  ↑/↓       - Navigate transactions"),
            Line::from("  l         - Add/edit label for selected transaction"),
            Line::from(""),
            Line::from(vec![
                Span::styled("Account Types:", Style::default().add_modifier(Modifier::BOLD))
            ]),
            Line::from("  Legacy       - Traditional Bitcoin addresses (1...)"),
            Line::from("  SegWit       - Segregated Witness addresses (3...)"),
            Line::from("  NativeSegWit - Bech32 addresses (bc1...)"),
        ];

        let help_text = Paragraph::new(text)
            .block(Block::default().borders(Borders::ALL).title("Help"));

        f.render_widget(help_text, area);
    }

    fn handle_normal_mode(&mut self, key: KeyEvent) -> Result<(), io::Error> {
        match key.code {
            KeyCode::Char('q') => {
                if key.modifiers.contains(KeyModifiers::CONTROL) {
                    return Err(io::Error::new(io::ErrorKind::Interrupted, "Quit"));
                }
                if self.current_tab != Tab::Help {
                    return Err(io::Error::new(io::ErrorKind::Interrupted, "Quit"));
                } else {
                    self.current_tab = Tab::Overview;
                }
            },
            KeyCode::Tab => {
                self.cycle_tab();
            },
            KeyCode::Char('?') => {
                if self.current_tab != Tab::Help {
                    self.current_tab = Tab::Help;
                } else {
                    self.current_tab = Tab::Overview;
                }
            },
            KeyCode::Char('o') => self.current_tab = Tab::Overview,
            KeyCode::Char('a') => {
                if self.current_tab == Tab::Accounts {
                    self.handle_generate_address()?;
                } else {
                    self.current_tab = Tab::Accounts;
                }
            },
            KeyCode::Char('t') => self.current_tab = Tab::Transactions,
            KeyCode::Char('n') => {
                if self.current_tab == Tab::Accounts {
                    self.input_mode = InputMode::AccountCreation;
                    self.input_text.clear();
                }
            },
            KeyCode::Char('l') => {
                if self.current_tab == Tab::Transactions {
                    self.handle_transaction_label_start()?;
                }
            },
            KeyCode::Down => {
                match self.current_tab {
                    Tab::Accounts => {
                        let accounts = self.wallet.list_accounts();
                        if !accounts.is_empty() {
                            let i = match self.accounts_state.selected() {
                                Some(i) => {
                                    if i >= accounts.len() - 1 {
                                        0
                                    } else {
                                        i + 1
                                    }
                                }
                                None => 0,
                            };
                            self.accounts_state.select(Some(i));
                        }
                    },
                    Tab::Transactions => {
                        let transactions = self.history.get_all_transactions();
                        if !transactions.is_empty() {
                            let i = match self.transactions_state.selected() {
                                Some(i) => {
                                    if i >= transactions.len() - 1 {
                                        0
                                    } else {
                                        i + 1
                                    }
                                }
                                None => 0,
                            };
                            self.transactions_state.select(Some(i));
                            self.selected_transaction = Some(transactions[i].hash.clone());
                        }
                    },
                    _ => {}
                }
            },
            KeyCode::Up => {
                match self.current_tab {
                    Tab::Accounts => {
                        let accounts = self.wallet.list_accounts();
                        if !accounts.is_empty() {
                            let i = match self.accounts_state.selected() {
                                Some(i) => {
                                    if i == 0 {
                                        accounts.len() - 1
                                    } else {
                                        i - 1
                                    }
                                }
                                None => 0,
                            };
                            self.accounts_state.select(Some(i));
                        }
                    },
                    Tab::Transactions => {
                        let transactions = self.history.get_all_transactions();
                        if !transactions.is_empty() {
                            let i = match self.transactions_state.selected() {
                                Some(i) => {
                                    if i == 0 {
                                        transactions.len() - 1
                                    } else {
                                        i - 1
                                    }
                                }
                                None => 0,
                            };
                            self.transactions_state.select(Some(i));
                            self.selected_transaction = Some(transactions[i].hash.clone());
                        }
                    },
                    _ => {}
                }
            },
            _ => {}
        }
        Ok(())
    }

    fn handle_account_creation_mode(&mut self, key: KeyEvent) -> Result<(), io::Error> {
        match key.code {
            KeyCode::Enter => {
                if !self.input_text.is_empty() {
                    self.create_account();
                }
                self.input_mode = InputMode::Normal;
            },
            KeyCode::Esc => {
                self.input_mode = InputMode::Normal;
                self.input_text.clear();
            },
            KeyCode::Char(c) => {
                self.input_text.push(c);
            },
            KeyCode::Backspace => {
                self.input_text.pop();
            },
            _ => {}
        }
        Ok(())
    }

    fn handle_transaction_labeling_mode(&mut self, key: KeyEvent) -> Result<(), io::Error> {
        match key.code {
            KeyCode::Enter => {
                self.apply_transaction_label();
                self.input_mode = InputMode::Normal;
            },
            KeyCode::Esc => {
                self.input_mode = InputMode::Normal;
                self.input_text.clear();
            },
            KeyCode::Char(c) => {
                self.input_text.push(c);
            },
            KeyCode::Backspace => {
                self.input_text.pop();
            },
            _ => {}
        }
        Ok(())
    }

    fn handle_address_display_mode(&mut self, key: KeyEvent) -> Result<(), io::Error> {
        match key.code {
            KeyCode::Enter | KeyCode::Esc => {
                self.input_mode = InputMode::Normal;
            },
            _ => {}
        }
        Ok(())
    }

    fn cycle_tab(&mut self) {
        self.current_tab = match self.current_tab {
            Tab::Overview => Tab::Accounts,
            Tab::Accounts => Tab::Transactions,
            Tab::Transactions => Tab::Help,
            Tab::Help => Tab::Overview,
        };
    }

    fn create_account(&mut self) {
        let account_name = self.input_text.trim().to_string();
        match self.wallet.create_account(account_name.clone(), AccountType::NativeSegWit) {
            Ok(_) => {
                self.message = Some(Message::Success(format!("Account '{}' created successfully", account_name)));
                // Select the newly created account - get account count after the mutable borrow is released
                let account_count = self.wallet.list_accounts().len();
                if account_count > 0 {
                    self.accounts_state.select(Some(account_count - 1));
                }
            },
            Err(e) => {
                self.message = Some(Message::Error(format!("Failed to create account: {}", e)));
            }
        }
        self.input_text.clear();
    }

    fn handle_generate_address(&mut self) -> Result<(), io::Error> {
        if let Some(idx) = self.accounts_state.selected() {
            // Collect account name first to avoid borrowing conflicts
            let account_name = {
                let accounts = self.wallet.list_accounts();
                if idx < accounts.len() {
                    Some(accounts[idx].1.name.clone())
                } else {
                    None
                }
            };
            
            if let Some(name) = account_name {
                match self.wallet.get_new_address(&name) {
                    Ok(address) => {
                        self.last_generated_address = Some(address);
                        self.input_mode = InputMode::AddressDisplay;
                    },
                    Err(e) => {
                        self.message = Some(Message::Error(format!("Failed to generate address: {}", e)));
                    }
                }
            }
        } else {
            self.message = Some(Message::Error("No account selected".to_string()));
        }
        Ok(())
    }

    fn handle_transaction_label_start(&mut self) -> Result<(), io::Error> {
        if let Some(idx) = self.transactions_state.selected() {
            let transactions = self.history.get_all_transactions();
            if idx < transactions.len() {
                let tx = transactions[idx];
                self.selected_transaction = Some(tx.hash.clone());
                self.input_mode = InputMode::TransactionLabeling;
                
                // Pre-fill with existing label
                if let Some(label) = &tx.label {
                    self.input_text = label.clone();
                } else {
                    self.input_text.clear();
                }
            }
        } else {
            self.message = Some(Message::Error("No transaction selected".to_string()));
        }
        Ok(())
    }

    fn apply_transaction_label(&mut self) {
        if let Some(tx_hash) = &self.selected_transaction {
            let label = self.input_text.trim().to_string();
            match self.history.add_transaction_label(tx_hash, label.clone()) {
                Ok(_) => {
                    self.message = Some(Message::Success(format!("Transaction labeled as '{}'", label)));
                },
                Err(e) => {
                    self.message = Some(Message::Error(format!("Failed to label transaction: {}", e)));
                }
            }
        } else {
            self.message = Some(Message::Error("No transaction selected".to_string()));
        }
        self.input_text.clear();
    }

    /// Create a test transaction (for development/demo purposes)
    #[cfg(test)]
    pub fn create_test_transaction(&mut self) -> Result<(), io::Error> {
        use crate::history::TransactionRecord;
        use chrono::Utc;
        
        let tx = TransactionRecord {
            hash: format!("test_tx_{}", Utc::now().timestamp()),
            timestamp: Utc::now(),
            direction: TransactionDirection::Received,
            amount: 50000,
            fee: 1000,
            status: TransactionStatus::Confirmed(6),
            label: None,
            category: None,
            tags: vec![],
        };
        
        match self.history.add_transaction(tx) {
            Ok(_) => {
                self.message = Some(Message::Success("Test transaction created".to_string()));
            },
            Err(e) => {
                self.message = Some(Message::Error(format!("Failed to create test transaction: {}", e)));
            }
        }
        
        Ok(())
    }
}