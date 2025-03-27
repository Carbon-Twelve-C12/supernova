use std::io;
use ratatui::{
    backend::CrosstermBackend,
    layout::{Constraint, Direction, Layout},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, List, ListItem, Paragraph},
    Frame, Terminal,
};
use crossterm::{
    event::{self, DisableMouseCapture, EnableMouseCapture, Event, KeyCode},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};

use crate::{
    hdwallet::{HDWallet, AccountType},
    history::{TransactionHistory, TransactionDirection, TransactionStatus},
};

pub struct WalletTui {
    wallet: HDWallet,
    history: TransactionHistory,
    current_tab: Tab,
    selected_account: Option<String>,
}

#[derive(PartialEq)]
enum Tab {
    Overview,
    Accounts,
    Transactions,
}

impl WalletTui {
    pub fn new(wallet: HDWallet, history: TransactionHistory) -> Result<Self, io::Error> {
        Ok(Self {
            wallet,
            history,
            current_tab: Tab::Overview,
            selected_account: None,
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
            terminal.draw(|f| {
                let chunks = Layout::default()
                    .direction(Direction::Vertical)
                    .margin(1)
                    .constraints([
                        Constraint::Length(3),
                        Constraint::Min(0),
                    ].as_ref())
                    .split(f.size());

                let tabs = vec!["Overview", "Accounts", "Transactions"];
                let tabs = tabs.iter().map(|t| {
                    let (first, rest) = t.split_at(1);
                    Line::from(vec![
                        Span::styled(first, Style::default().fg(Color::Yellow)),
                        Span::styled(rest, Style::default().fg(Color::White)),
                    ])
                }).collect::<Vec<_>>();

                let tabs = ratatui::widgets::Tabs::new(tabs)
                    .block(Block::default().borders(Borders::ALL).title("Tabs"))
                    .select(match self.current_tab {
                        Tab::Overview => 0,
                        Tab::Accounts => 1,
                        Tab::Transactions => 2,
                    })
                .style(Style::default().fg(Color::White))
                    .highlight_style(Style::default().fg(Color::Yellow));

                f.render_widget(tabs, chunks[0]);

                match self.current_tab {
                    Tab::Overview => self.render_overview(f, chunks[1]),
                    Tab::Accounts => self.render_accounts(f, chunks[1]),
                    Tab::Transactions => self.render_transactions(f, chunks[1]),
                }
            })?;

                if let Event::Key(key) = event::read()? {
                    match key.code {
                    KeyCode::Char('q') => return Ok(()),
                    KeyCode::Char('o') => self.current_tab = Tab::Overview,
                    KeyCode::Char('a') => self.current_tab = Tab::Accounts,
                    KeyCode::Char('t') => self.current_tab = Tab::Transactions,
                    KeyCode::Char('n') => {
                        if let Some(account_name) = &self.selected_account {
                            if let Ok(address) = self.wallet.get_new_address(account_name) {
                                // Address created successfully
                            }
                        }
                    }
                    KeyCode::Char('c') => {
                        // Create new account logic
                        if let Ok(()) = self.wallet.create_account(
                            format!("Account {}", self.wallet.list_accounts().len()),
                            AccountType::NativeSegWit,
                        ) {
                            // Account created successfully
                        }
                    }
                    _ => {}
                }
            }
        }
    }

    fn render_overview(&self, f: &mut Frame, area: ratatui::layout::Rect) {
        let total_balance = self.wallet.get_total_balance().unwrap_or(0);
        let total_sent = self.history.get_total_sent();
        let total_received = self.history.get_total_received();
        let net_flow = self.history.get_net_flow();

        let text = vec![
            Line::from(vec![
                Span::raw("Total Balance: "),
                Span::styled(format!("{} sats", total_balance), Style::default().fg(Color::Green)),
            ]),
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
        ];

        let overview = Paragraph::new(text)
            .block(Block::default().borders(Borders::ALL).title("Overview"));

        f.render_widget(overview, area);
    }

    fn render_accounts(&self, f: &mut Frame, area: ratatui::layout::Rect) {
        let accounts = self.wallet.list_accounts();
        let mut items = Vec::new();

        for (index, account) in accounts {
            let balance = self.wallet.get_balance(&account.name).unwrap_or(0);
            items.push(ListItem::new(vec![Line::from(vec![
                Span::raw(format!("{}. ", index)),
                Span::styled(&account.name, Style::default().fg(Color::Yellow)),
                Span::raw(" - "),
                Span::styled(format!("{} sats", balance), Style::default().fg(Color::Green)),
            ])]));
        }

        let accounts_list = List::new(items)
            .block(Block::default().borders(Borders::ALL).title("Accounts"))
            .highlight_style(Style::default().add_modifier(Modifier::BOLD));

        f.render_widget(accounts_list, area);
    }

    fn render_transactions(&self, f: &mut Frame, area: ratatui::layout::Rect) {
        let transactions = self.history.get_recent_transactions(10);
        let mut items = Vec::new();

        for tx in transactions {
            let amount_color = match tx.direction {
                TransactionDirection::Sent => Color::Red,
                TransactionDirection::Received => Color::Green,
            };

            let status_color = match tx.status {
                TransactionStatus::Pending => Color::Yellow,
                TransactionStatus::Confirmed(_) => Color::Green,
                TransactionStatus::Failed => Color::Red,
            };

            items.push(ListItem::new(vec![Line::from(vec![
                Span::styled(
                    tx.timestamp.format("%Y-%m-%d %H:%M").to_string(),
                    Style::default().fg(Color::Blue),
                ),
                Span::raw(" - "),
                Span::styled(
                    format!("{} sats", tx.amount),
                    Style::default().fg(amount_color),
                ),
                Span::raw(" - "),
                Span::styled(
                    format!("{:?}", tx.status),
                    Style::default().fg(status_color),
                ),
            ])]));
        }

        let transactions_list = List::new(items)
            .block(Block::default().borders(Borders::ALL).title("Recent Transactions"))
            .highlight_style(Style::default().add_modifier(Modifier::BOLD));

        f.render_widget(transactions_list, area);
    }
}