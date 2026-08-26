use std::env;
use std::path::{Path, PathBuf};
use std::process;

use fresnica_client::{
    balance_asset_label, operation_summary, BalanceSnapshot, FresnicaClient, HistorySnapshot,
    PaymentRequest, PreparedPayment, PreparedTrustline, TrustlineAction, TrustlineRequest,
    WalletRecord,
};
use ratatui::crossterm::event::{self, Event, KeyCode, KeyEventKind};
use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::style::{Modifier, Style};
use ratatui::text::Line;
use ratatui::widgets::{Block, Clear, List, ListItem, Paragraph, Row, Table};
use ratatui::Frame;
use serde_json::Value;
use zeroize::Zeroize;

const HELP: &str = r#"Fresnica native Rust TUI

Usage:
  fresnica-tui [--home PATH] [--network mainnet|testnet] [--wallet NAME]

Keys:
  q / Esc     quit
  r           refresh balances and recent activity
  [ / ]       previous / next wallet on the selected network
  s           prepare a payment from the selected signing wallet
  t           add, change, or remove an issued-asset trustline

Write flow:
  form -> shared service preparation -> review -> passcode -> SDK/Core signing -> Horizon

The Rust TUI is an engineering/reference UI over fresnica-client. It does not
implement separate wallet, cryptographic, transaction, or Horizon semantics.
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

    ratatui::run(|terminal| -> std::io::Result<()> {
        loop {
            terminal.draw(|frame| app.render(frame))?;
            match event::read()? {
                Event::Key(key) if key.kind == KeyEventKind::Press => {
                    if app.handle_key(key.code) {
                        break Ok(());
                    }
                }
                _ => {}
            }
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

enum Mode {
    Browse,
    Send(SendForm),
    Trustline(TrustlineForm),
    PaymentReview(PreparedPayment),
    TrustlineReview(PreparedTrustline),
    Passcode {
        prepared: PreparedWrite,
        passcode: String,
    },
}

#[derive(Clone)]
enum PreparedWrite {
    Payment(PreparedPayment),
    Trustline(PreparedTrustline),
}

struct SendForm {
    amount: String,
    asset: String,
    destination: String,
    memo: String,
    active: usize,
}

impl SendForm {
    fn new() -> Self {
        Self {
            amount: String::new(),
            asset: "XLM".to_owned(),
            destination: String::new(),
            memo: String::new(),
            active: 0,
        }
    }

    fn current_mut(&mut self) -> &mut String {
        match self.active {
            0 => &mut self.amount,
            1 => &mut self.asset,
            2 => &mut self.destination,
            _ => &mut self.memo,
        }
    }

    fn request(&self, wallet: &str) -> PaymentRequest {
        PaymentRequest {
            wallet: Some(wallet.to_owned()),
            amount: self.amount.clone(),
            asset: self.asset.clone(),
            destination: self.destination.clone(),
            memo: (!self.memo.is_empty()).then(|| self.memo.clone()),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum TrustlineFormAction {
    Add,
    SetLimit,
    Remove,
}

impl TrustlineFormAction {
    fn label(self) -> &'static str {
        match self {
            Self::Add => "add",
            Self::SetLimit => "limit",
            Self::Remove => "remove",
        }
    }

    fn previous(self) -> Self {
        match self {
            Self::Add => Self::Remove,
            Self::SetLimit => Self::Add,
            Self::Remove => Self::SetLimit,
        }
    }

    fn next(self) -> Self {
        match self {
            Self::Add => Self::SetLimit,
            Self::SetLimit => Self::Remove,
            Self::Remove => Self::Add,
        }
    }
}

struct TrustlineForm {
    action: TrustlineFormAction,
    asset: String,
    limit: String,
    active: usize,
}

impl TrustlineForm {
    fn new() -> Self {
        Self {
            action: TrustlineFormAction::Add,
            asset: String::new(),
            limit: String::new(),
            active: 0,
        }
    }

    fn current_mut(&mut self) -> Option<&mut String> {
        match self.active {
            1 => Some(&mut self.asset),
            2 => Some(&mut self.limit),
            _ => None,
        }
    }

    fn request(&self, wallet: &str) -> TrustlineRequest {
        let action = match self.action {
            TrustlineFormAction::Add => TrustlineAction::Add {
                limit: (!self.limit.is_empty()).then(|| self.limit.clone()),
            },
            TrustlineFormAction::SetLimit => TrustlineAction::SetLimit {
                limit: self.limit.clone(),
            },
            TrustlineFormAction::Remove => TrustlineAction::Remove,
        };
        TrustlineRequest {
            wallet: Some(wallet.to_owned()),
            asset: self.asset.clone(),
            action,
        }
    }
}

struct App {
    client: FresnicaClient,
    wallets: Vec<WalletRecord>,
    selected: usize,
    balances: Vec<Value>,
    operations: Vec<Value>,
    status: String,
    mode: Mode,
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
            mode: Mode::Browse,
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

    fn handle_key(&mut self, code: KeyCode) -> bool {
        let wallet_name = self.selected_wallet().name.clone();
        let wallet_watch_only = self.selected_wallet().watch_only();
        let mut payment_request = None;
        let mut trustline_request = None;
        let mut submit = false;

        match &mut self.mode {
            Mode::Browse => match code {
                KeyCode::Char('q') | KeyCode::Esc => return true,
                KeyCode::Char('r') => self.refresh(),
                KeyCode::Char('[') | KeyCode::Left => self.select_previous(),
                KeyCode::Char(']') | KeyCode::Right => self.select_next(),
                KeyCode::Char('s') => {
                    if wallet_watch_only {
                        self.status =
                            "Selected wallet is watch-only; attach a signer before sending"
                                .to_owned();
                    } else {
                        self.mode = Mode::Send(SendForm::new());
                        self.status = "Preparing payment".to_owned();
                    }
                }
                KeyCode::Char('t') => {
                    if wallet_watch_only {
                        self.status =
                            "Selected wallet is watch-only; attach a signer before changing trustlines"
                                .to_owned();
                    } else {
                        self.mode = Mode::Trustline(TrustlineForm::new());
                        self.status = "Preparing trustline change".to_owned();
                    }
                }
                _ => {}
            },
            Mode::Send(form) => match code {
                KeyCode::Esc => {
                    self.mode = Mode::Browse;
                    self.status = "Payment cancelled before review".to_owned();
                }
                KeyCode::Tab | KeyCode::Down => form.active = (form.active + 1) % 4,
                KeyCode::BackTab | KeyCode::Up => form.active = (form.active + 3) % 4,
                KeyCode::Enter if form.active < 3 => form.active += 1,
                KeyCode::Enter => payment_request = Some(form.request(&wallet_name)),
                KeyCode::Backspace => {
                    form.current_mut().pop();
                }
                KeyCode::Char(character) => form.current_mut().push(character),
                _ => {}
            },
            Mode::Trustline(form) => match code {
                KeyCode::Esc => {
                    self.mode = Mode::Browse;
                    self.status = "Trustline change cancelled before review".to_owned();
                }
                KeyCode::Tab | KeyCode::Down => form.active = (form.active + 1) % 3,
                KeyCode::BackTab | KeyCode::Up => form.active = (form.active + 2) % 3,
                KeyCode::Left if form.active == 0 => form.action = form.action.previous(),
                KeyCode::Right if form.active == 0 => form.action = form.action.next(),
                KeyCode::Char('a') if form.active == 0 => form.action = TrustlineFormAction::Add,
                KeyCode::Char('l') if form.active == 0 => {
                    form.action = TrustlineFormAction::SetLimit
                }
                KeyCode::Char('r') if form.active == 0 => form.action = TrustlineFormAction::Remove,
                KeyCode::Enter if form.active == 0 => form.active = 1,
                KeyCode::Enter
                    if form.active == 1 && form.action == TrustlineFormAction::Remove =>
                {
                    trustline_request = Some(form.request(&wallet_name));
                }
                KeyCode::Enter if form.active == 1 => form.active = 2,
                KeyCode::Enter => trustline_request = Some(form.request(&wallet_name)),
                KeyCode::Backspace => {
                    if let Some(value) = form.current_mut() {
                        value.pop();
                    }
                }
                KeyCode::Char(character) => {
                    if let Some(value) = form.current_mut() {
                        value.push(character);
                    }
                }
                _ => {}
            },
            Mode::PaymentReview(prepared) => match code {
                KeyCode::Char('y') | KeyCode::Enter => {
                    self.mode = Mode::Passcode {
                        prepared: PreparedWrite::Payment(prepared.clone()),
                        passcode: String::new(),
                    };
                    self.status = "Enter Fresnica passcode; input is masked".to_owned();
                }
                KeyCode::Char('n') | KeyCode::Esc => {
                    self.mode = Mode::Browse;
                    self.status = "Payment cancelled after review".to_owned();
                }
                _ => {}
            },
            Mode::TrustlineReview(prepared) => match code {
                KeyCode::Char('y') | KeyCode::Enter => {
                    self.mode = Mode::Passcode {
                        prepared: PreparedWrite::Trustline(prepared.clone()),
                        passcode: String::new(),
                    };
                    self.status = "Enter Fresnica passcode; input is masked".to_owned();
                }
                KeyCode::Char('n') | KeyCode::Esc => {
                    self.mode = Mode::Browse;
                    self.status = "Trustline change cancelled after review".to_owned();
                }
                _ => {}
            },
            Mode::Passcode { passcode, .. } => match code {
                KeyCode::Esc => {
                    passcode.zeroize();
                    self.mode = Mode::Browse;
                    self.status = "Transaction cancelled before signing".to_owned();
                }
                KeyCode::Enter if passcode.is_empty() => {
                    self.status = "Fresnica passcode cannot be empty".to_owned();
                }
                KeyCode::Enter => submit = true,
                KeyCode::Backspace => {
                    passcode.pop();
                }
                KeyCode::Char(character) => passcode.push(character),
                _ => {}
            },
        }

        if let Some(request) = payment_request {
            match self.client.prepare_payment(&request) {
                Ok(prepared) => {
                    self.mode = Mode::PaymentReview(prepared);
                    self.status = "Review the exact prepared payment before signing".to_owned();
                }
                Err(error) => self.status = error,
            }
        }

        if let Some(request) = trustline_request {
            match self.client.prepare_trustline(&request) {
                Ok(prepared) => {
                    self.mode = Mode::TrustlineReview(prepared);
                    self.status =
                        "Review the exact prepared trustline change before signing".to_owned();
                }
                Err(error) => self.status = error,
            }
        }

        if submit {
            let result = match &mut self.mode {
                Mode::Passcode { prepared, passcode } => {
                    let submitted_passcode = passcode.clone();
                    passcode.zeroize();
                    match prepared {
                        PreparedWrite::Payment(prepared) => {
                            self.client.submit_payment(prepared, submitted_passcode)
                        }
                        PreparedWrite::Trustline(prepared) => {
                            self.client.submit_trustline(prepared, submitted_passcode)
                        }
                    }
                }
                _ => return false,
            };
            match result {
                Ok(submission) => {
                    self.mode = Mode::Browse;
                    self.refresh();
                    self.status = match submission.ledger {
                        Some(ledger) => format!("Submitted {} in ledger {ledger}", submission.hash),
                        None => format!("Submitted {}", submission.hash),
                    };
                }
                Err(error) if error.contains("invalid Fresnica passcode") => {
                    self.status = error;
                }
                Err(error) => {
                    self.mode = Mode::Browse;
                    self.status = error;
                }
            }
        }

        false
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

        match &self.mode {
            Mode::Browse => {}
            Mode::Send(form) => self.render_send_form(frame, form),
            Mode::Trustline(form) => self.render_trustline_form(frame, form),
            Mode::PaymentReview(prepared) => self.render_payment_review(frame, prepared),
            Mode::TrustlineReview(prepared) => self.render_trustline_review(frame, prepared),
            Mode::Passcode { passcode, .. } => self.render_passcode(frame, passcode),
        }
    }

    fn render_header(&self, frame: &mut Frame, area: Rect) {
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

    fn render_assets(&self, frame: &mut Frame, area: Rect) {
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

    fn render_activity(&self, frame: &mut Frame, area: Rect) {
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

    fn render_footer(&self, frame: &mut Frame, area: Rect) {
        let help = match &self.mode {
            Mode::Browse => "q quit   r refresh   [ / ] switch wallet   s send   t trustline",
            Mode::Send(_) => "type value   Tab/Up/Down field   Enter next/prepare   Esc cancel",
            Mode::Trustline(_) => {
                "action: Left/Right or a/l/r   Tab field   Enter next/prepare   Esc cancel"
            }
            Mode::PaymentReview(_) | Mode::TrustlineReview(_) => "y/Enter sign   n/Esc cancel",
            Mode::Passcode { .. } => "Enter submit   Backspace edit   Esc cancel",
        };
        let body = format!("{}\n{help}", self.status);
        frame.render_widget(Paragraph::new(body).block(Block::bordered()), area);
    }

    fn render_send_form(&self, frame: &mut Frame, form: &SendForm) {
        let area = popup_area(frame.area());
        let fields = [
            ("Amount", &form.amount),
            ("Asset", &form.asset),
            ("Destination", &form.destination),
            ("Memo (optional)", &form.memo),
        ];
        let lines = fields
            .iter()
            .enumerate()
            .map(|(index, (label, value))| {
                let line = Line::from(format!("{label:<18} {value}"));
                if index == form.active {
                    line.style(Style::new().add_modifier(Modifier::BOLD))
                } else {
                    line
                }
            })
            .chain(std::iter::once(Line::from("")))
            .chain(std::iter::once(Line::from(
                "Destination may be a G address or a saved contact name.",
            )))
            .collect::<Vec<_>>();
        frame.render_widget(Clear, area);
        frame.render_widget(
            Paragraph::new(lines).block(Block::bordered().title("Prepare payment")),
            area,
        );
    }

    fn render_trustline_form(&self, frame: &mut Frame, form: &TrustlineForm) {
        let area = popup_area(frame.area());
        let limit_value = if form.action == TrustlineFormAction::Remove {
            "(not used)"
        } else if form.limit.is_empty() && form.action == TrustlineFormAction::Add {
            "(default Fresnica limit)"
        } else {
            form.limit.as_str()
        };
        let fields = [
            ("Action", form.action.label()),
            ("Asset", form.asset.as_str()),
            ("Limit", limit_value),
        ];
        let lines = fields
            .iter()
            .enumerate()
            .map(|(index, (label, value))| {
                let line = Line::from(format!("{label:<18} {value}"));
                if index == form.active {
                    line.style(Style::new().add_modifier(Modifier::BOLD))
                } else {
                    line
                }
            })
            .chain(std::iter::once(Line::from("")))
            .chain(std::iter::once(Line::from(
                "Asset format: CODE:GISSUER. Add may leave limit empty for the default.",
            )))
            .collect::<Vec<_>>();
        frame.render_widget(Clear, area);
        frame.render_widget(
            Paragraph::new(lines).block(Block::bordered().title("Prepare trustline change")),
            area,
        );
    }

    fn render_payment_review(&self, frame: &mut Frame, prepared: &PreparedPayment) {
        let review = &prepared.review;
        let mut lines = vec![
            Line::from(format!("Operation: {}", review.operation.label())),
            Line::from(format!(
                "From:      {} ({})",
                review.wallet_name, review.source
            )),
            Line::from(match &review.contact_name {
                Some(name) => format!("To:        {name} ({})", review.destination),
                None => format!("To:        {}", review.destination),
            }),
            Line::from(format!("Amount:    {} {}", review.amount, review.asset)),
            Line::from(format!("Fee:       {} XLM", review.fee_xlm)),
            Line::from(format!("Network:   {}", review.network)),
        ];
        if let Some(memo) = &review.memo {
            lines.push(Line::from(format!(
                "Memo:      {} ({})",
                memo.value, memo.memo_type
            )));
        }
        lines.push(Line::from(""));
        lines.push(Line::from("Press y/Enter to continue to signing."));
        let area = popup_area(frame.area());
        frame.render_widget(Clear, area);
        frame.render_widget(
            Paragraph::new(lines).block(Block::bordered().title("Review transaction")),
            area,
        );
    }

    fn render_trustline_review(&self, frame: &mut Frame, prepared: &PreparedTrustline) {
        let review = &prepared.review;
        let mut lines = vec![
            Line::from(format!(
                "Operation: ChangeTrust ({})",
                review.operation.label()
            )),
            Line::from(format!(
                "Wallet:    {} ({})",
                review.wallet_name, review.source
            )),
            Line::from(format!("Asset:     {}", review.asset)),
            Line::from(format!("Fee:       {} XLM", review.fee_xlm)),
            Line::from(format!("Network:   {}", review.network)),
        ];
        if let Some(limit) = &review.limit {
            lines.insert(3, Line::from(format!("Limit:     {limit}")));
        }
        lines.push(Line::from(""));
        lines.push(Line::from("Press y/Enter to continue to signing."));
        let area = popup_area(frame.area());
        frame.render_widget(Clear, area);
        frame.render_widget(
            Paragraph::new(lines).block(Block::bordered().title("Review trustline change")),
            area,
        );
    }

    fn render_passcode(&self, frame: &mut Frame, passcode: &str) {
        let area = popup_area(frame.area());
        let masked = "*".repeat(passcode.chars().count());
        let lines = vec![
            Line::from("The passcode is passed to the shared client service only after review."),
            Line::from(""),
            Line::from(format!("Fresnica passcode: {masked}")),
            Line::from(""),
            Line::from("Enter submits the prepared transaction."),
        ];
        frame.render_widget(Clear, area);
        frame.render_widget(
            Paragraph::new(lines).block(Block::bordered().title("Sign and submit")),
            area,
        );
    }
}

fn popup_area(area: Rect) -> Rect {
    let [_, vertical, _] = Layout::vertical([
        Constraint::Percentage(14),
        Constraint::Percentage(72),
        Constraint::Percentage(14),
    ])
    .areas(area);
    let [_, popup, _] = Layout::horizontal([
        Constraint::Percentage(10),
        Constraint::Percentage(80),
        Constraint::Percentage(10),
    ])
    .areas(vertical);
    popup
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

    #[test]
    fn send_form_builds_shared_payment_request() {
        let mut form = SendForm::new();
        form.amount = "1.25".to_owned();
        form.destination = "Alice".to_owned();
        form.memo = "hello".to_owned();
        let request = form.request("primary");
        assert_eq!(request.wallet.as_deref(), Some("primary"));
        assert_eq!(request.amount, "1.25");
        assert_eq!(request.asset, "XLM");
        assert_eq!(request.destination, "Alice");
        assert_eq!(request.memo.as_deref(), Some("hello"));
    }

    #[test]
    fn trustline_form_builds_shared_service_request() {
        let mut form = TrustlineForm::new();
        form.action = TrustlineFormAction::SetLimit;
        form.asset = "USD:GAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAWHF".to_owned();
        form.limit = "2500".to_owned();
        let request = form.request("primary");
        assert_eq!(request.wallet.as_deref(), Some("primary"));
        assert_eq!(
            request.action,
            TrustlineAction::SetLimit {
                limit: "2500".to_owned()
            }
        );
    }
}
