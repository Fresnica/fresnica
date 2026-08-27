use std::env;
use std::path::{Path, PathBuf};
use std::process;
use std::time::{SystemTime, UNIX_EPOCH};

use fresnica_client::{
    balance_asset_label, operation_summary, BalanceSnapshot, CandleSnapshot, FresnicaClient,
    HistorySnapshot, OfferRequest, OfferReviewDetails, OfferSide, OpenOffer, OrderBookSnapshot,
    PairTradesSnapshot, PaymentRequest, PreparedOffer, PreparedPayment, PreparedTrustline,
    TrustlineAction, TrustlineRequest, WalletRecord,
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
  r           refresh balances, offers, and recent activity
  [ / ]       previous / next wallet on the selected network
  s           prepare a payment from the selected signing wallet
  t           manage issued-asset trustlines
  d           open the DEX market selector

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
    Offer(OfferForm),
    Market(MarketForm),
    MarketView(MarketSnapshot),
    PaymentReview(PreparedPayment),
    TrustlineReview(PreparedTrustline),
    OfferReview(PreparedOffer),
    Passcode {
        prepared: PreparedWrite,
        passcode: String,
    },
}

struct MarketSnapshot {
    order_book: OrderBookSnapshot,
    trades: PairTradesSnapshot,
    candles: CandleSnapshot,
}

struct MarketForm {
    base: String,
    counter: String,
    active: usize,
}

impl MarketForm {
    fn new() -> Self {
        Self {
            base: String::new(),
            counter: "XLM".to_owned(),
            active: 0,
        }
    }

    fn current_mut(&mut self) -> &mut String {
        if self.active == 0 {
            &mut self.base
        } else {
            &mut self.counter
        }
    }

    fn pair(&self) -> (String, String) {
        (self.base.clone(), self.counter.clone())
    }
}

#[derive(Clone)]
enum PreparedWrite {
    Payment(PreparedPayment),
    Trustline(PreparedTrustline),
    Offer(PreparedOffer),
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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum OfferFormAction {
    Buy,
    Sell,
    Update,
    Cancel,
}

impl OfferFormAction {
    fn label(self) -> &'static str {
        match self {
            Self::Buy => "buy",
            Self::Sell => "sell",
            Self::Update => "update",
            Self::Cancel => "cancel",
        }
    }

    fn previous(self) -> Self {
        match self {
            Self::Buy => Self::Cancel,
            Self::Sell => Self::Buy,
            Self::Update => Self::Sell,
            Self::Cancel => Self::Update,
        }
    }

    fn next(self) -> Self {
        match self {
            Self::Buy => Self::Sell,
            Self::Sell => Self::Update,
            Self::Update => Self::Cancel,
            Self::Cancel => Self::Buy,
        }
    }
}

struct OfferForm {
    action: OfferFormAction,
    offer_id: String,
    base: String,
    counter: String,
    amount: String,
    price: String,
    allow_trustline: bool,
    active: usize,
}

impl OfferForm {
    fn new() -> Self {
        Self {
            action: OfferFormAction::Buy,
            offer_id: String::new(),
            base: String::new(),
            counter: "XLM".to_owned(),
            amount: String::new(),
            price: String::new(),
            allow_trustline: false,
            active: 0,
        }
    }

    fn for_market(action: OfferFormAction, base: String, counter: String) -> Self {
        let mut form = Self::new();
        form.action = action;
        form.base = base;
        form.counter = counter;
        form.active = match action {
            OfferFormAction::Buy | OfferFormAction::Sell => 3,
            OfferFormAction::Update | OfferFormAction::Cancel => 1,
        };
        form
    }

    fn field_count(&self) -> usize {
        if self.action == OfferFormAction::Cancel {
            2
        } else {
            6
        }
    }

    fn next_field(&mut self) {
        self.active = (self.active + 1) % self.field_count();
    }

    fn previous_field(&mut self) {
        self.active = (self.active + self.field_count() - 1) % self.field_count();
    }

    fn normalize_active(&mut self) {
        if self.active >= self.field_count() {
            self.active = self.field_count() - 1;
        }
    }

    fn current_mut(&mut self) -> Option<&mut String> {
        match (self.action, self.active) {
            (OfferFormAction::Cancel, 1) => Some(&mut self.offer_id),
            (OfferFormAction::Update, 1) => Some(&mut self.offer_id),
            (OfferFormAction::Update, 2) => Some(&mut self.base),
            (OfferFormAction::Update, 3) => Some(&mut self.counter),
            (OfferFormAction::Update, 4) => Some(&mut self.amount),
            (OfferFormAction::Update, 5) => Some(&mut self.price),
            (OfferFormAction::Buy | OfferFormAction::Sell, 1) => Some(&mut self.base),
            (OfferFormAction::Buy | OfferFormAction::Sell, 2) => Some(&mut self.counter),
            (OfferFormAction::Buy | OfferFormAction::Sell, 3) => Some(&mut self.amount),
            (OfferFormAction::Buy | OfferFormAction::Sell, 4) => Some(&mut self.price),
            _ => None,
        }
    }

    fn request(&self, wallet: &str) -> Result<OfferRequest, String> {
        let wallet = Some(wallet.to_owned());
        match self.action {
            OfferFormAction::Buy | OfferFormAction::Sell => Ok(OfferRequest::Create {
                wallet,
                side: if self.action == OfferFormAction::Buy {
                    OfferSide::Buy
                } else {
                    OfferSide::Sell
                },
                base: self.base.clone(),
                counter: self.counter.clone(),
                amount: self.amount.clone(),
                price: self.price.clone(),
                allow_trustline: self.allow_trustline,
            }),
            OfferFormAction::Update => Ok(OfferRequest::Update {
                wallet,
                offer_id: self.offer_id()?,
                base: self.base.clone(),
                counter: self.counter.clone(),
                amount: self.amount.clone(),
                price: self.price.clone(),
            }),
            OfferFormAction::Cancel => Ok(OfferRequest::Cancel {
                wallet,
                offer_id: self.offer_id()?,
            }),
        }
    }

    fn offer_id(&self) -> Result<i64, String> {
        self.offer_id
            .parse::<i64>()
            .ok()
            .filter(|value| *value > 0)
            .ok_or_else(|| "offer id must be a positive integer".to_owned())
    }
}

struct App {
    client: FresnicaClient,
    wallets: Vec<WalletRecord>,
    selected: usize,
    balances: Vec<Value>,
    operations: Vec<Value>,
    offers: Vec<OpenOffer>,
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
            offers: Vec::new(),
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
        let offers_result = self.client.open_offers(Some(&name), 8);

        let mut failures = Vec::new();
        match balance_result {
            Ok(BalanceSnapshot { balances, .. }) => self.balances = balances,
            Err(error) => failures.push(format!("balances: {error}")),
        }
        match history_result {
            Ok(HistorySnapshot { operations, .. }) => self.operations = operations,
            Err(error) => failures.push(format!("activity: {error}")),
        }
        match offers_result {
            Ok(snapshot) => self.offers = snapshot.offers,
            Err(error) => failures.push(format!("offers: {error}")),
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
        let mut offer_request = None;
        let mut market_request = None;
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
                KeyCode::Char('d') => {
                    self.mode = Mode::Market(MarketForm::new());
                    self.status = "Choose a DEX market pair".to_owned();
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
                KeyCode::Char('x') if form.active == 0 => form.action = TrustlineFormAction::Remove,
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
            Mode::Offer(form) => match code {
                KeyCode::Esc => {
                    self.mode = Mode::Browse;
                    self.status = "Offer change cancelled before review".to_owned();
                }
                KeyCode::Tab | KeyCode::Down => form.next_field(),
                KeyCode::BackTab | KeyCode::Up => form.previous_field(),
                KeyCode::Left if form.active == 0 => {
                    form.action = form.action.previous();
                    form.normalize_active();
                }
                KeyCode::Right if form.active == 0 => {
                    form.action = form.action.next();
                    form.normalize_active();
                }
                KeyCode::Char('b') if form.active == 0 => form.action = OfferFormAction::Buy,
                KeyCode::Char('s') if form.active == 0 => form.action = OfferFormAction::Sell,
                KeyCode::Char('e') if form.active == 0 => {
                    form.action = OfferFormAction::Update;
                    form.normalize_active();
                }
                KeyCode::Char('x') if form.active == 0 => {
                    form.action = OfferFormAction::Cancel;
                    form.normalize_active();
                }
                KeyCode::Left | KeyCode::Right | KeyCode::Char(' ')
                    if form.active == 5
                        && matches!(form.action, OfferFormAction::Buy | OfferFormAction::Sell) =>
                {
                    form.allow_trustline = !form.allow_trustline;
                }
                KeyCode::Enter if form.active + 1 < form.field_count() => form.next_field(),
                KeyCode::Enter => match form.request(&wallet_name) {
                    Ok(request) => offer_request = Some(request),
                    Err(error) => self.status = error,
                },
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
            Mode::Market(form) => match code {
                KeyCode::Esc => {
                    self.mode = Mode::Browse;
                    self.status = "Market selection cancelled".to_owned();
                }
                KeyCode::Tab | KeyCode::Down => form.active = (form.active + 1) % 2,
                KeyCode::BackTab | KeyCode::Up => form.active = (form.active + 1) % 2,
                KeyCode::Enter if form.active == 0 => form.active = 1,
                KeyCode::Enter => market_request = Some(form.pair()),
                KeyCode::Backspace => {
                    form.current_mut().pop();
                }
                KeyCode::Char(character) => form.current_mut().push(character),
                _ => {}
            },
            Mode::MarketView(snapshot) => match code {
                KeyCode::Esc => {
                    self.mode = Mode::Browse;
                    self.status = "Closed SDEX market".to_owned();
                }
                KeyCode::Char('r') => {
                    market_request = Some((
                        snapshot.order_book.base.clone(),
                        snapshot.order_book.counter.clone(),
                    ));
                }
                KeyCode::Char('w') => {
                    market_request = Some((
                        snapshot.order_book.counter.clone(),
                        snapshot.order_book.base.clone(),
                    ));
                }
                KeyCode::Char('b')
                | KeyCode::Char('s')
                | KeyCode::Char('e')
                | KeyCode::Char('x') => {
                    if wallet_watch_only {
                        self.status =
                            "Selected wallet is watch-only; attach a signer before managing offers"
                                .to_owned();
                    } else {
                        let action = match code {
                            KeyCode::Char('b') => OfferFormAction::Buy,
                            KeyCode::Char('s') => OfferFormAction::Sell,
                            KeyCode::Char('e') => OfferFormAction::Update,
                            KeyCode::Char('x') => OfferFormAction::Cancel,
                            _ => unreachable!(),
                        };
                        let base = snapshot.order_book.base.clone();
                        let counter = snapshot.order_book.counter.clone();
                        self.mode = Mode::Offer(OfferForm::for_market(action, base, counter));
                        self.status = match action {
                            OfferFormAction::Buy => "Preparing DEX buy offer",
                            OfferFormAction::Sell => "Preparing DEX sell offer",
                            OfferFormAction::Update => "Enter the offer ID to edit",
                            OfferFormAction::Cancel => "Enter the offer ID to cancel",
                        }
                        .to_owned();
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
            Mode::OfferReview(prepared) => match code {
                KeyCode::Char('y') | KeyCode::Enter => {
                    self.mode = Mode::Passcode {
                        prepared: PreparedWrite::Offer(prepared.clone()),
                        passcode: String::new(),
                    };
                    self.status = "Enter Fresnica passcode; input is masked".to_owned();
                }
                KeyCode::Char('n') | KeyCode::Esc => {
                    self.mode = Mode::Browse;
                    self.status = "Offer change cancelled after review".to_owned();
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

        if let Some(request) = offer_request {
            match self.client.prepare_offer(&request) {
                Ok(prepared) => {
                    self.mode = Mode::OfferReview(prepared);
                    self.status = "Review the exact prepared SDEX offer before signing".to_owned();
                }
                Err(error) => self.status = error,
            }
        }

        if let Some((base, counter)) = market_request {
            let snapshot = (|| {
                let order_book = self.client.order_book(&base, &counter)?;
                let trades = self.client.pair_trades(&base, &counter, 8)?;
                let end_time = SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .map_err(|_| "system clock is before Unix epoch".to_owned())?
                    .as_millis() as u64;
                let candles = self.client.candles(
                    &base,
                    &counter,
                    3_600_000,
                    Some(end_time.saturating_sub(24 * 3_600_000)),
                    Some(end_time),
                    None,
                    6,
                )?;
                Ok::<_, String>(MarketSnapshot {
                    order_book,
                    trades,
                    candles,
                })
            })();
            match snapshot {
                Ok(snapshot) => {
                    self.status = format!(
                        "Updated SDEX market {}/{}",
                        snapshot.order_book.base, snapshot.order_book.counter
                    );
                    self.mode = Mode::MarketView(snapshot);
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
                        PreparedWrite::Offer(prepared) => {
                            self.client.submit_offer(prepared, submitted_passcode)
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
            Constraint::Length(4),
        ])
        .areas(frame.area());

        self.render_header(frame, header_area);
        if main_area.width >= 100 {
            let [portfolio_area, activity_area] =
                Layout::horizontal([Constraint::Percentage(68), Constraint::Percentage(32)])
                    .areas(main_area);
            let [assets_area, offers_area] =
                Layout::vertical([Constraint::Percentage(60), Constraint::Percentage(40)])
                    .areas(portfolio_area);
            self.render_assets(frame, assets_area);
            self.render_offers(frame, offers_area);
            self.render_activity(frame, activity_area);
        } else {
            let [assets_area, offers_area, activity_area] = Layout::vertical([
                Constraint::Percentage(45),
                Constraint::Percentage(25),
                Constraint::Percentage(30),
            ])
            .areas(main_area);
            self.render_assets(frame, assets_area);
            self.render_offers(frame, offers_area);
            self.render_activity(frame, activity_area);
        }
        self.render_footer(frame, footer_area);

        match &self.mode {
            Mode::Browse => {}
            Mode::Send(form) => self.render_send_form(frame, form),
            Mode::Trustline(form) => self.render_trustline_form(frame, form),
            Mode::Offer(form) => self.render_offer_form(frame, form),
            Mode::Market(form) => self.render_market_form(frame, form),
            Mode::MarketView(snapshot) => self.render_market_view(frame, snapshot),
            Mode::PaymentReview(prepared) => self.render_payment_review(frame, prepared),
            Mode::TrustlineReview(prepared) => self.render_trustline_review(frame, prepared),
            Mode::OfferReview(prepared) => self.render_offer_review(frame, prepared),
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

    fn render_offers(&self, frame: &mut Frame, area: Rect) {
        let header = Row::new(["ID", "Selling", "Buying", "Amount", "Price"])
            .style(Style::new().add_modifier(Modifier::BOLD));
        let rows = self.offers.iter().map(|offer| {
            Row::new([
                offer.offer_id.to_string(),
                compact_asset(&offer.selling),
                compact_asset(&offer.buying),
                offer.amount.clone(),
                offer.price.clone(),
            ])
        });
        let table = Table::new(
            rows,
            [
                Constraint::Length(10),
                Constraint::Min(14),
                Constraint::Min(14),
                Constraint::Length(14),
                Constraint::Length(12),
            ],
        )
        .header(header)
        .column_spacing(1)
        .block(Block::bordered().title("Open offers"));
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
            Mode::Browse => {
                "q quit   r refresh   [ / ] switch wallet   s send   t manage assets   d DEX"
            }
            Mode::Send(_) => "type value   Tab/Up/Down field   Enter next/prepare   Esc cancel",
            Mode::Trustline(_) => {
                "action: Left/Right or a/l/x   Tab field   Enter next/prepare   Esc cancel"
            }
            Mode::Offer(_) => {
                "action: Left/Right or b/s/e/x   Tab field   Space toggles trustline   Enter next/prepare"
            }
            Mode::Market(_) => "type asset   Tab/Up/Down field   Enter next/open   Esc cancel",
            Mode::MarketView(_) => {
                "r refresh   w swap pair   b buy   s sell   e edit   x cancel   Esc back"
            }
            Mode::PaymentReview(_) | Mode::TrustlineReview(_) | Mode::OfferReview(_) => {
                "y/Enter sign   n/Esc cancel"
            }
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

    fn render_offer_form(&self, frame: &mut Frame, form: &OfferForm) {
        let area = popup_area(frame.area());
        let fields = if form.action == OfferFormAction::Cancel {
            vec![
                ("Action", form.action.label().to_owned()),
                ("Offer ID", form.offer_id.clone()),
            ]
        } else if form.action == OfferFormAction::Update {
            vec![
                ("Action", form.action.label().to_owned()),
                ("Offer ID", form.offer_id.clone()),
                ("Base", form.base.clone()),
                ("Counter", form.counter.clone()),
                ("Amount", form.amount.clone()),
                ("Price", form.price.clone()),
            ]
        } else {
            vec![
                ("Action", form.action.label().to_owned()),
                ("Base", form.base.clone()),
                ("Counter", form.counter.clone()),
                ("Amount", form.amount.clone()),
                ("Price", form.price.clone()),
                (
                    "Add trustline",
                    if form.allow_trustline { "yes" } else { "no" }.to_owned(),
                ),
            ]
        };
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
                "Action: b buy, s sell, u update, c cancel. Price is counter per base.",
            )))
            .collect::<Vec<_>>();
        frame.render_widget(Clear, area);
        frame.render_widget(
            Paragraph::new(lines).block(Block::bordered().title("Prepare SDEX offer")),
            area,
        );
    }

    fn render_market_form(&self, frame: &mut Frame, form: &MarketForm) {
        let area = popup_area(frame.area());
        let fields = [
            ("Base", form.base.clone()),
            ("Counter", form.counter.clone()),
        ];
        let lines = fields
            .iter()
            .enumerate()
            .map(|(index, (label, value))| {
                let line = Line::from(format!("{label:<12} {value}"));
                if index == form.active {
                    line.style(Style::new().add_modifier(Modifier::BOLD))
                } else {
                    line
                }
            })
            .chain(std::iter::once(Line::from("")))
            .chain(std::iter::once(Line::from(
                "Assets are XLM or full CODE:GISSUER identities. Price is counter per base.",
            )))
            .collect::<Vec<_>>();
        frame.render_widget(Clear, area);
        frame.render_widget(
            Paragraph::new(lines).block(Block::bordered().title("Open SDEX market")),
            area,
        );
    }

    fn render_market_view(&self, frame: &mut Frame, snapshot: &MarketSnapshot) {
        let area = popup_area(frame.area());
        let [book_area, trades_area, candles_area] = Layout::vertical([
            Constraint::Percentage(54),
            Constraint::Percentage(23),
            Constraint::Percentage(23),
        ])
        .areas(area);
        let book = &snapshot.order_book;
        let header = Row::new(["Amount", "Price", "Price", "Amount"])
            .style(Style::new().add_modifier(Modifier::BOLD));
        let count = book.bids.len().max(book.asks.len());
        let rows = (0..count).map(|index| {
            let bid = book.bids.get(index);
            let ask = book.asks.get(index);
            Row::new([
                bid.map(|level| level.amount.clone()).unwrap_or_default(),
                bid.map(|level| level.price.clone()).unwrap_or_default(),
                ask.map(|level| level.price.clone()).unwrap_or_default(),
                ask.map(|level| level.amount.clone()).unwrap_or_default(),
            ])
        });
        let title = format!(
            "BID · BUY   {} / {}   ASK · SELL",
            compact_asset(&book.base),
            compact_asset(&book.counter)
        );
        let book_table = Table::new(
            rows,
            [
                Constraint::Percentage(25),
                Constraint::Percentage(25),
                Constraint::Percentage(25),
                Constraint::Percentage(25),
            ],
        )
        .header(header)
        .column_spacing(1)
        .block(Block::bordered().title(title));

        let trade_rows = snapshot.trades.trades.iter().map(|trade| {
            Row::new([
                trade.ledger_close_time.clone().unwrap_or_default(),
                trade.base_side.label().to_owned(),
                trade.base_amount.clone(),
                trade.price.clone(),
            ])
        });
        let trades_table = Table::new(
            trade_rows,
            [
                Constraint::Percentage(40),
                Constraint::Percentage(12),
                Constraint::Percentage(24),
                Constraint::Percentage(24),
            ],
        )
        .header(
            Row::new(["Time", "Side", "Base amount", "Price"])
                .style(Style::new().add_modifier(Modifier::BOLD)),
        )
        .column_spacing(1)
        .block(Block::bordered().title("Recent trades"));

        let candle_rows = snapshot.candles.candles.iter().map(|candle| {
            Row::new([
                candle.timestamp.to_string(),
                candle.open.clone(),
                candle.high.clone(),
                candle.low.clone(),
                candle.close.clone(),
                candle.base_volume.clone(),
                candle.trade_count.to_string(),
            ])
        });
        let candles_table = Table::new(
            candle_rows,
            [
                Constraint::Percentage(20),
                Constraint::Percentage(13),
                Constraint::Percentage(13),
                Constraint::Percentage(13),
                Constraint::Percentage(13),
                Constraint::Percentage(18),
                Constraint::Percentage(10),
            ],
        )
        .header(
            Row::new([
                "Time(ms)", "Open", "High", "Low", "Close", "Volume", "Trades",
            ])
            .style(Style::new().add_modifier(Modifier::BOLD)),
        )
        .column_spacing(1)
        .block(Block::bordered().title("1h candles · last 24h"));

        frame.render_widget(Clear, area);
        frame.render_widget(book_table, book_area);
        frame.render_widget(trades_table, trades_area);
        frame.render_widget(candles_table, candles_area);
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
        if let Some(authorization) = review.authorization {
            lines.insert(
                lines.len().saturating_sub(2),
                Line::from(format!("Auth:      {}", authorization.label())),
            );
        }
        if let Some(clawback_enabled) = review.clawback_enabled {
            lines.insert(
                lines.len().saturating_sub(2),
                Line::from(format!(
                    "Clawback:  {}",
                    if clawback_enabled {
                        "enabled"
                    } else {
                        "disabled"
                    }
                )),
            );
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

    fn render_offer_review(&self, frame: &mut Frame, prepared: &PreparedOffer) {
        let review = &prepared.review;
        let mut lines = vec![
            Line::from(format!(
                "Operation: {} ({})",
                review.operation.label(),
                review.action.label()
            )),
            Line::from(format!(
                "Wallet:    {} ({})",
                review.wallet_name, review.source
            )),
        ];
        if let Some(offer_id) = review.offer_id {
            lines.push(Line::from(format!("Offer:     #{offer_id}")));
        }
        match &review.details {
            OfferReviewDetails::Trade {
                side,
                base,
                counter,
                amount,
                price,
                requested_price,
                price_n,
                price_d,
                total,
                trustline_asset,
                trustline_limit,
            } => {
                lines.push(Line::from(format!("Side:      {}", side.label())));
                lines.push(Line::from(format!("Pair:      {base} / {counter}")));
                lines.push(Line::from(format!("Amount:    {amount} {base}")));
                lines.push(Line::from(format!("Price:     {price} {counter}/{base}")));
                lines.push(Line::from(format!("Encoded:   {price_n}/{price_d}")));
                if let Some(requested) = requested_price {
                    lines.push(Line::from(format!(
                        "Requested: {requested} {counter}/{base}"
                    )));
                }
                lines.push(Line::from(format!("Total:     {total} {counter}")));
                if let Some(asset) = trustline_asset {
                    let limit = trustline_limit
                        .as_deref()
                        .map(|value| format!("; limit {value}"))
                        .unwrap_or_default();
                    lines.push(Line::from(format!(
                        "Trustline: + {asset}{limit} (explicitly approved)"
                    )));
                }
            }
            OfferReviewDetails::Cancel { selling, buying } => {
                lines.push(Line::from(format!("Selling:   {selling}")));
                lines.push(Line::from(format!("Buying:    {buying}")));
            }
        }
        lines.push(Line::from(format!("Fee:       {} XLM", review.fee_xlm)));
        lines.push(Line::from(format!("Network:   {}", review.network)));
        lines.push(Line::from(""));
        lines.push(Line::from("Press y/Enter to continue to signing."));
        let area = popup_area(frame.area());
        frame.render_widget(Clear, area);
        frame.render_widget(
            Paragraph::new(lines).block(Block::bordered().title("Review SDEX offer")),
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

fn compact_asset(value: &str) -> String {
    let Some((code, issuer)) = value.split_once(':') else {
        return value.to_owned();
    };
    if issuer.len() <= 14 {
        return value.to_owned();
    }
    format!("{code}:{}...{}", &issuer[..6], &issuer[issuer.len() - 4..])
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
    fn market_offer_form_uses_dex_pair_and_action_focus() {
        let buy = OfferForm::for_market(
            OfferFormAction::Buy,
            "XLM".to_owned(),
            "USD:GISSUER".to_owned(),
        );
        assert_eq!(buy.action, OfferFormAction::Buy);
        assert_eq!(buy.base, "XLM");
        assert_eq!(buy.counter, "USD:GISSUER");
        assert_eq!(buy.active, 3);

        let edit = OfferForm::for_market(
            OfferFormAction::Update,
            "XLM".to_owned(),
            "USD:GISSUER".to_owned(),
        );
        assert_eq!(edit.action, OfferFormAction::Update);
        assert_eq!(edit.active, 1);
    }

    #[test]
    fn offer_form_builds_shared_create_request() {
        let mut form = OfferForm::new();
        form.action = OfferFormAction::Sell;
        form.base = "XLM".to_owned();
        form.counter = "USD:GAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAWHF".to_owned();
        form.amount = "4.5".to_owned();
        form.price = "1.25".to_owned();
        form.allow_trustline = true;
        let request = form.request("primary").unwrap();
        assert!(matches!(
            request,
            OfferRequest::Create {
                wallet: Some(ref wallet),
                side: OfferSide::Sell,
                allow_trustline: true,
                ref amount,
                ref price,
                ..
            } if wallet == "primary" && amount == "4.5" && price == "1.25"
        ));
    }

    #[test]
    fn offer_form_builds_shared_cancel_request() {
        let mut form = OfferForm::new();
        form.action = OfferFormAction::Cancel;
        form.offer_id = "42".to_owned();
        assert!(matches!(
            form.request("primary").unwrap(),
            OfferRequest::Cancel {
                wallet: Some(ref wallet),
                offer_id: 42,
            } if wallet == "primary"
        ));
    }

    #[test]
    fn offer_form_builds_shared_update_request() {
        let mut form = OfferForm::new();
        form.action = OfferFormAction::Update;
        form.offer_id = "42".to_owned();
        form.base = "XLM".to_owned();
        form.counter = "USD:GAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAWHF".to_owned();
        form.amount = "3.5".to_owned();
        form.price = "1.75".to_owned();
        assert!(matches!(
            form.request("primary").unwrap(),
            OfferRequest::Update {
                wallet: Some(ref wallet),
                offer_id: 42,
                ref amount,
                ref price,
                ..
            } if wallet == "primary" && amount == "3.5" && price == "1.75"
        ));
    }

    #[test]
    fn market_form_keeps_full_asset_pair_identity() {
        let mut form = MarketForm::new();
        form.base = "USD:GAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAWHF".to_owned();
        let (base, counter) = form.pair();
        assert_eq!(
            base,
            "USD:GAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAWHF"
        );
        assert_eq!(counter, "XLM");
    }

    #[test]
    fn compact_asset_keeps_code_and_shortens_only_issuer() {
        let asset = "USD:GAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAWHF";
        assert_eq!(compact_asset("XLM"), "XLM");
        assert_eq!(compact_asset(asset), "USD:GAAAAA...AWHF");
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
