use std::env;
use std::path::{Path, PathBuf};
use std::process;

use fresnica_client::{
    balance_asset_label, operation_summary, BalanceSnapshot, FresnicaClient, HistorySnapshot,
    WalletRecord,
};
use ratatui::crossterm::event::{self, Event, KeyCode, KeyEventKind};
use ratatui::layout::{Constraint, Layout};
use ratatui::style::{Modifier, Style};
use ratatui::text::Line;
use ratatui::widgets::{Block, List, ListItem, Paragraph, Row, Table};
use ratatui::Frame;
use serde_json::Value;

const HELP: &str = r#"Fresnica native Rust TUI

Usage:
  fresnica-tui [--home PATH] [--network mainnet|testnet] [--wallet NAME]

Keys:
  q / Esc     quit
  r           refresh balances and recent activity
  [ / ]       previous / next wallet on the selected network

The Rust TUI is an engineering/reference UI over fresnica-client. It does not
implement separate wallet, cryptographic, or Horizon semantics.
"#;

fn main() {
    if let Err(error) = run() {
        eprintln!("Error: {error}");
        process::exit(2);
    }
}

fn run() -> Result<(), String> {
    let arguments: Vec<String> = env::args().skip(1).collect();
    if arguments == ["--help"] || arguments == ["-h"] {
        print!("{HELP}");
        return Ok(());
    }
    if arguments == ["--version"] || arguments == ["-V"] {
        println!("fresnica-tui {}", env!("CARGO_PKG_VERSION"));
        return Ok(());
    }

    let options = Options::parse(&arguments)?;
    let client = FresnicaClient::new(&options.home, &options.network)?;
    let mut app = App::new(client, options.wallet.as_deref())?;

    ratatui::run(|mut terminal| loop {
        terminal.draw(|frame| app.render(frame))?;
        match event::read()? {
            Event::Key(key) if key.kind == KeyEventKind::Press => match key.code {
                KeyCode::Char('q') | KeyCode::Esc => break Ok(()),
                KeyCode::Char('r') => app.refresh(),
                KeyCode::Char('[') | KeyCode::Left => app.select_previous(),
                KeyCode::Char(']') | KeyCode::Right => app.select_next(),
                _ => {}
            },
            _ => {}
        }
    })
    .map_err(|error| format!("terminal error: {error}"))
}

#[derive(Debug)]
struct Options {
    home: PathBuf,
    network: String,
    wallet: Option<String>,
}

impl Options {
    fn parse(arguments: &[String]) -> Result<Self, String> {
        let mut home = None;
        let mut network = "mainnet".to_owned();
        let mut wallet = None;
        let mut index = 0;
        while index < arguments.len() {
            match arguments[index].as_str() {
                "--home" => {
                    index += 1;
                    home = Some(expand_path(
                        arguments
                            .get(index)
                            .ok_or_else(|| "--home requires a path".to_owned())?,
                    )?);
                    index += 1;
                }
                "--network" => {
                    index += 1;
                    network = arguments
                        .get(index)
                        .ok_or_else(|| "--network requires mainnet or testnet".to_owned())?
                        .to_owned();
                    if !matches!(network.as_str(), "mainnet" | "testnet") {
                        return Err(format!("unknown network: {network}"));
                    }
                    index += 1;
                }
                "--wallet" => {
                    index += 1;
                    wallet = Some(
                        arguments
                            .get(index)
                            .ok_or_else(|| "--wallet requires a name".to_owned())?
                            .to_owned(),
                    );
                    index += 1;
                }
                other => return Err(format!("unknown option: {other}\n\n{HELP}")),
            }
        }
        let home = match home {
            Some(home) => home,
            None => default_home()?,
        };
        Ok(Self {
            home,
            network,
            wallet,
        })
    }
}

struct App {
    client: FresnicaClient,
    wallets: Vec<WalletRecord>,
    selected: usize,
    balances: Vec<Value>,
    operations: Vec<Value>,
    status: String,
}

impl App {
    fn new(client: FresnicaClient, requested_wallet: Option<&str>) -> Result<Self, String> {
        let wallets = client.wallets()?;
        if wallets.is_empty() {
            return Err(format!(
                "no {} wallets are available in {}",
                client.network(),
                client.storage().home().display()
            ));
        }

        let selected = match requested_wallet {
            Some(name) => wallets
                .iter()
                .position(|wallet| wallet.name == name)
                .ok_or_else(|| format!("wallet not found on {}: {name}", client.network()))?,
            None => {
                let resolved = client.resolve_wallet(None)?;
                wallets
                    .iter()
                    .position(|wallet| wallet.name == resolved.name)
                    .ok_or_else(|| {
                        format!(
                            "default wallet \"{}\" is not on {}",
                            resolved.name,
                            client.network()
                        )
                    })?
            }
        };

        let mut app = Self {
            client,
            wallets,
            selected,
            balances: Vec::new(),
            operations: Vec::new(),
            status: String::new(),
        };
        app.refresh();
        Ok(app)
    }

    fn selected_wallet(&self) -> &WalletRecord {
        &self.wallets[self.selected]
    }

    fn refresh(&mut self) {
        let name = self.selected_wallet().name.clone();
        let balance_result = self.client.balances(Some(&name));
        let history_result = self.client.history(Some(&name), 12);

        let mut failures = Vec::new();
        match balance_result {
            Ok(BalanceSnapshot { balances, .. }) => self.balances = balances,
            Err(error) => failures.push(format!("balances: {error}")),
        }
        match history_result {
            Ok(HistorySnapshot { operations, .. }) => self.operations = operations,
            Err(error) => failures.push(format!("activity: {error}")),
        }

        self.status = if failures.is_empty() {
            "Updated from Horizon".to_owned()
        } else {
            failures.join(" · ")
        };
    }

    fn select_previous(&mut self) {
        if self.wallets.len() > 1 {
            self.selected = if self.selected == 0 {
                self.wallets.len() - 1
            } else {
                self.selected - 1
            };
            self.refresh();
        }
    }

    fn select_next(&mut self) {
        if self.wallets.len() > 1 {
            self.selected = (self.selected + 1) % self.wallets.len();
            self.refresh();
        }
    }

    fn render(&self, frame: &mut Frame) {
        let [header_area, main_area, footer_area] = Layout::vertical([
            Constraint::Length(4),
            Constraint::Min(8),
            Constraint::Length(3),
        ])
        .areas(frame.area());

        self.render_header(frame, header_area);
        if main_area.width >= 100 {
            let [assets_area, activity_area] =
                Layout::horizontal([Constraint::Percentage(56), Constraint::Percentage(44)])
                    .areas(main_area);
            self.render_assets(frame, assets_area);
            self.render_activity(frame, activity_area);
        } else {
            let [assets_area, activity_area] =
                Layout::vertical([Constraint::Percentage(55), Constraint::Percentage(45)])
                    .areas(main_area);
            self.render_assets(frame, assets_area);
            self.render_activity(frame, activity_area);
        }
        self.render_footer(frame, footer_area);
    }

    fn render_header(&self, frame: &mut Frame, area: ratatui::layout::Rect) {
        let wallet = self.selected_wallet();
        let capability = if wallet.watch_only() {
            "Watch-only"
        } else {
            "Local signer"
        };
        let title = format!(
            "{}  [{}]  {}  ({}/{})",
            wallet.name,
            wallet.network,
            capability,
            self.selected + 1,
            self.wallets.len()
        );
        let body = format!(
            "{}\n{}",
            wallet.address, "Fresnica Rust TUI · shared client/service layer"
        );
        frame.render_widget(
            Paragraph::new(body).block(Block::bordered().title(title)),
            area,
        );
    }

    fn render_assets(&self, frame: &mut Frame, area: ratatui::layout::Rect) {
        let header = Row::new(["Asset", "Balance", "Selling", "Buying"])
            .style(Style::new().add_modifier(Modifier::BOLD));
        let rows = self.balances.iter().map(|balance| {
            Row::new([
                balance_asset_label(balance),
                text(balance, "balance").unwrap_or("0").to_owned(),
                text(balance, "selling_liabilities")
                    .unwrap_or("0")
                    .to_owned(),
                text(balance, "buying_liabilities")
                    .unwrap_or("0")
                    .to_owned(),
            ])
        });
        let table = Table::new(
            rows,
            [
                Constraint::Min(24),
                Constraint::Length(16),
                Constraint::Length(14),
                Constraint::Length(14),
            ],
        )
        .header(header)
        .column_spacing(1)
        .block(Block::bordered().title("Assets"));
        frame.render_widget(table, area);
    }

    fn render_activity(&self, frame: &mut Frame, area: ratatui::layout::Rect) {
        let address = &self.selected_wallet().address;
        let items = if self.operations.is_empty() {
            vec![ListItem::new("No recent activity")]
        } else {
            self.operations
                .iter()
                .map(|operation| {
                    let created_at = text(operation, "created_at").unwrap_or("?");
                    let operation_type = text(operation, "type").unwrap_or("unknown");
                    ListItem::new(Line::from(format!(
                        "{created_at}  {operation_type}  {}",
                        operation_summary(operation, address)
                    )))
                })
                .collect()
        };
        frame.render_widget(
            List::new(items).block(Block::bordered().title("Recent activity")),
            area,
        );
    }

    fn render_footer(&self, frame: &mut Frame, area: ratatui::layout::Rect) {
        let body = format!(
            "{}\nq quit   r refresh   [ / ] switch wallet (session only)",
            self.status
        );
        frame.render_widget(Paragraph::new(body).block(Block::bordered()), area);
    }
}

fn text<'a>(value: &'a Value, key: &str) -> Option<&'a str> {
    value.get(key).and_then(Value::as_str)
}

fn default_home() -> Result<PathBuf, String> {
    if let Some(home) = env::var_os("FRESNICA_HOME") {
        return expand_path(&home.to_string_lossy());
    }
    let base = env::var_os("HOME")
        .or_else(|| env::var_os("USERPROFILE"))
        .ok_or_else(|| "unable to determine home directory; set FRESNICA_HOME".to_owned())?;
    Ok(PathBuf::from(base).join(".fresnica"))
}

fn expand_path(value: &str) -> Result<PathBuf, String> {
    if value == "~" {
        return env::var_os("HOME")
            .or_else(|| env::var_os("USERPROFILE"))
            .map(PathBuf::from)
            .ok_or_else(|| "unable to expand ~; set HOME or USERPROFILE".to_owned());
    }
    if let Some(rest) = value.strip_prefix("~/") {
        let home = env::var_os("HOME")
            .or_else(|| env::var_os("USERPROFILE"))
            .ok_or_else(|| "unable to expand ~; set HOME or USERPROFILE".to_owned())?;
        return Ok(PathBuf::from(home).join(rest));
    }
    Ok(Path::new(value).to_path_buf())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_network_and_wallet_options() {
        let options = Options::parse(&[
            "--home".to_owned(),
            "/tmp/fresnica".to_owned(),
            "--network".to_owned(),
            "testnet".to_owned(),
            "--wallet".to_owned(),
            "alpha".to_owned(),
        ])
        .unwrap();
        assert_eq!(options.home, PathBuf::from("/tmp/fresnica"));
        assert_eq!(options.network, "testnet");
        assert_eq!(options.wallet.as_deref(), Some("alpha"));
    }

    #[test]
    fn rejects_unknown_network_before_starting_terminal() {
        let error = Options::parse(&["--network".to_owned(), "future".to_owned()]).unwrap_err();
        assert_eq!(error, "unknown network: future");
    }
}
