"""Pair-scoped SDEX presentation for the state-driven Fresnica TUI."""

from dataclasses import dataclass
from decimal import Decimal, InvalidOperation, localcontext
from typing import Literal

from rich.text import Text
from textual import work
from textual.app import ComposeResult
from textual.binding import Binding
from textual.containers import Horizontal, Vertical
from textual.screen import ModalScreen
from textual.widgets import Button, DataTable, Footer, Input, Label, Static

from ..errors import FresnicaError
from ..market_discovery import MarketDiscoveryService
from ..models import Asset, MarketPair, OpenOffer
from ..offer_service import offer_view_for_pair
from ..presentation import format_amount, format_timestamp, short_address
from ..sdex_presentation import (
    format_market_price,
    offer_id_label,
    stellar_decimal_parts,
    stellar_price_ratio_parts,
)
from ..trade_segments import account_trade_segment_for_pair
from .asset_picker import AssetPickerDialog


DexActionKind = Literal["create", "update", "cancel"]
MarketTab = Literal["popular", "favorites"]


@dataclass(frozen=True)
class DexOfferAction:
    kind: DexActionKind
    pair: MarketPair
    side: Literal["buy", "sell"] | None = None
    amount: str | None = None
    price: str | None = None
    offer: OpenOffer | None = None


class MarketPairDialog(ModalScreen[MarketPair | None]):
    """Fex-style market entry: Popular / Starred first, Add Pair separately."""

    BINDINGS = [
        Binding("escape", "cancel", "Back"),
        Binding("enter", "open_selected", "Open", priority=True),
        Binding("p", "popular", "Popular"),
        Binding("f", "favorites", "Starred"),
        Binding("a", "add_pair", "Add pair"),
        Binding("space", "toggle_favorite", "Star / Unstar"),
    ]

    CSS = """
    MarketPairDialog { align: center middle; }
    MarketPairDialog > #dialog { width: 112; height: 86%; padding: 1 2; border: round $accent; background: $surface; }
    #market-tabs { height: auto; margin-bottom: 1; }
    #market-tabs Button { margin-right: 1; }
    #market-status, #market-recent { height: 2; color: $text-muted; }
    #market-list { height: 1fr; min-height: 10; }
    #market-help { height: 2; color: $text-muted; margin-top: 1; }
    """

    def __init__(self):
        super().__init__()
        self._scope: tuple[str, str] | None = None
        self._tab: MarketTab = "popular"
        self._pairs: list[MarketPair] = []
        self._popular: list[MarketPair] = []
        self._catalog_entries = []
        self._pending_base: Asset | None = None

    def compose(self) -> ComposeResult:
        with Vertical(id="dialog"):
            yield Label("Stellar DEX markets")
            with Horizontal(id="market-tabs"):
                yield Button(Text("Popular [P]"), id="popular", variant="primary")
                yield Button(Text("★ Starred [F]"), id="favorites")
                yield Button(Text("Add pair [A]"), id="add-pair")
                yield Button(Text("Back [Esc]"), id="cancel")
            yield Static("Loading market list...", id="market-status")
            yield Static("Recent: —", id="market-recent")
            yield DataTable(id="market-list")
            yield Static(
                "Enter opens · Space stars/un-stars · price is COUNTER/BASE and order amount is BASE.",
                id="market-help",
            )
        yield Footer()

    def on_mount(self) -> None:
        table = self.query_one("#market-list", DataTable)
        if not table.columns:
            table.add_columns("★", "Pair", "Base source", "Counter source")
        table.cursor_type = "row"
        try:
            session = self.app.runtime.wallet_manager.view()
            self._scope = (session.record.network, session.wallet.address())
        except (FresnicaError, ValueError):
            self._scope = None
        self._load_cached_catalog()
        self._load_cached_popular()
        self._render_market_list()
        self._load_popular()

    def action_cancel(self) -> None:
        self.dismiss(None)

    def action_open_selected(self) -> None:
        pair = self._selected_pair()
        if pair is not None:
            self._open_pair(pair)

    def action_popular(self) -> None:
        self._tab = "popular"
        self._render_market_list()

    def action_favorites(self) -> None:
        self._tab = "favorites"
        self._render_market_list()

    def action_add_pair(self) -> None:
        self._pending_base = None
        self.app.push_screen(
            AssetPickerDialog(self.app.runtime, allow_native=True, title="Choose BASE asset"),
            self._base_selected,
        )

    def _base_selected(self, asset: Asset | None) -> None:
        if asset is None:
            return
        self._pending_base = asset
        self.app.push_screen(
            AssetPickerDialog(self.app.runtime, allow_native=True, title="Choose COUNTER asset"),
            self._counter_selected,
        )

    def _counter_selected(self, asset: Asset | None) -> None:
        base = self._pending_base
        self._pending_base = None
        if asset is None or base is None:
            return
        if asset == base:
            self.query_one("#market-status", Static).update(
                "BASE and COUNTER must be different · press A to choose again"
            )
            return
        self._open_pair(MarketPair(base, asset))

    def action_toggle_favorite(self) -> None:
        pair = self._selected_pair()
        store = getattr(self.app.runtime, "market_preferences", None)
        if pair is None or self._scope is None or store is None:
            return
        network, address = self._scope
        preferences = store.toggle_favorite(network, address, pair)
        starred = pair in preferences.favorites
        self._render_market_list(
            f"{'Starred' if starred else 'Unstarred'} {_pair_label(pair)}"
        )

    def on_data_table_row_selected(self, event: DataTable.RowSelected) -> None:
        if event.data_table.id == "market-list":
            self.action_open_selected()

    def on_button_pressed(self, event: Button.Pressed) -> None:
        actions = {
            "cancel": self.action_cancel,
            "popular": self.action_popular,
            "favorites": self.action_favorites,
            "add-pair": self.action_add_pair,
        }
        action = actions.get(event.button.id)
        if action is not None:
            action()

    def _open_pair(self, pair: MarketPair) -> None:
        store = getattr(self.app.runtime, "market_preferences", None)
        if self._scope is not None and store is not None:
            network, address = self._scope
            store.touch(network, address, pair)
        self.dismiss(pair)

    def _load_cached_catalog(self) -> None:
        if self._scope is None:
            return
        catalog = getattr(self.app.runtime, "asset_catalog", None)
        if catalog is None:
            return
        try:
            self._catalog_entries = list(catalog.cached(self._scope[0]))
        except (FresnicaError, ValueError):
            self._catalog_entries = []

    def _load_cached_popular(self) -> None:
        if self._scope is None:
            return
        catalog = getattr(self.app.runtime, "asset_catalog", None)
        if catalog is None:
            return
        try:
            self._popular = MarketDiscoveryService(catalog).popular_pairs(
                self._scope[0],
                limit=12,
                held_assets=self._held_assets(),
                refresh=False,
            )
        except (FresnicaError, ValueError):
            self._popular = []

    @work(exclusive=True, thread=True, exit_on_error=False)
    def _load_popular(self) -> None:
        if self._scope is None:
            return
        catalog = getattr(self.app.runtime, "asset_catalog", None)
        if catalog is None:
            return
        network, _ = self._scope
        try:
            pairs = MarketDiscoveryService(catalog).popular_pairs(
                network,
                limit=12,
                held_assets=self._held_assets(),
                refresh=True,
            )
            entries = list(catalog.cached(network))
            self.app.call_from_thread(self._apply_popular, pairs, entries, None)
        except (FresnicaError, ValueError) as exc:
            self.app.call_from_thread(self._apply_popular, [], [], exc)

    def _apply_popular(self, pairs, entries, error) -> None:
        if not self.is_mounted:
            return
        if entries:
            self._catalog_entries = list(entries)
        if pairs:
            self._popular = list(pairs)
        if error is not None:
            self._render_market_list(
                "Popular refresh unavailable · cached Popular, Starred and Add pair remain available"
            )
        else:
            self._render_market_list()

    def _render_market_list(self, status: str | None = None) -> None:
        preferences = self._preferences()
        favorites = preferences.favorites if preferences is not None else ()
        pairs = list(self._popular if self._tab == "popular" else favorites)
        self._pairs = pairs

        table = self.query_one("#market-list", DataTable)
        table.clear()
        favorite_set = set(favorites)
        for pair in pairs:
            table.add_row(
                "★" if pair in favorite_set else "",
                _pair_label(pair),
                self._asset_source(pair.base),
                self._asset_source(pair.counter),
            )

        self.query_one("#popular", Button).variant = (
            "primary" if self._tab == "popular" else "default"
        )
        self.query_one("#favorites", Button).variant = (
            "primary" if self._tab == "favorites" else "default"
        )
        recent = list(preferences.recents[:4]) if preferences is not None else []
        recent_text = " · ".join(_pair_label(pair) for pair in recent) or "—"
        self.query_one("#market-recent", Static).update(f"Recent: {recent_text}")

        if status is not None:
            message = status
        elif self._tab == "favorites" and not pairs:
            message = "Starred · no markets yet · switch to Popular [P] and press Space to star one"
        elif self._tab == "popular" and not pairs:
            message = "Popular · loading recommendations · Add pair [A] is always available"
        else:
            message = (
                f"{'Popular' if self._tab == 'popular' else 'Starred'} · "
                f"{len(pairs)} markets · Enter open · Space star/unstar"
            )
        self.query_one("#market-status", Static).update(message)

    def _preferences(self):
        store = getattr(self.app.runtime, "market_preferences", None)
        if store is None or self._scope is None:
            return None
        return store.get(self._scope[0], self._scope[1])

    def _held_assets(self) -> list[Asset]:
        try:
            session = self.app.runtime.wallet_manager.view()
            service = self.app.runtime.services_for(session.record.network).balance_service
            getter = getattr(service, "get_cached_portfolio_views", None)
            if getter is None:
                return []
            balances, _ = getter(session.wallet)
        except (FresnicaError, ValueError):
            return []
        return [
            item.asset
            for item in balances
            if not item.asset.is_liquidity_pool and item.balance > 0
        ]

    def _asset_source(self, asset: Asset) -> str:
        if asset.is_native:
            return "Stellar native"
        for entry in self._catalog_entries:
            if entry.asset == asset:
                return entry.domain or short_address(asset.issuer)
        return short_address(asset.issuer)

    def _selected_pair(self) -> MarketPair | None:
        if not self._pairs:
            return None
        row = self.query_one("#market-list", DataTable).cursor_row
        return self._pairs[max(0, min(row, len(self._pairs) - 1))]


class OfferFormDialog(ModalScreen[DexOfferAction | None]):
    BINDINGS = [("escape", "cancel", "Cancel")]

    CSS = """
    OfferFormDialog { align: center middle; }
    OfferFormDialog > #dialog { width: 92; height: auto; padding: 1 2; border: round $accent; background: $surface; }
    OfferFormDialog Input { margin-top: 1; }
    OfferFormDialog #market { color: $text-muted; margin-top: 1; }
    OfferFormDialog #form-error { color: $error; margin-top: 1; }
    OfferFormDialog #actions { height: auto; margin-top: 1; align-horizontal: right; }
    OfferFormDialog Button { margin-left: 1; }
    """

    def __init__(
        self,
        pair: MarketPair,
        side: Literal["buy", "sell"],
        offer: OpenOffer | None = None,
    ):
        super().__init__()
        self.pair = pair
        self.side = side
        self.offer = offer
        view = offer_view_for_pair(offer, pair) if offer is not None else None
        self.initial_amount = format_amount(view.amount) if view is not None else ""
        self.initial_price = format_market_price(view.price) if view is not None else ""

    def compose(self) -> ComposeResult:
        action = "Update" if self.offer is not None else "Create"
        with Vertical(id="dialog"):
            yield Label(f"{action} {self.side.upper()} limit offer")
            yield Static(
                f"{_asset_identity(self.pair.base)} / {_asset_identity(self.pair.counter)}\n"
                f"Amount: {_asset_code(self.pair.base)} · Price: {_asset_code(self.pair.counter)}/{_asset_code(self.pair.base)}",
                id="market",
            )
            yield Input(value=self.initial_amount, placeholder="Base amount", id="amount")
            yield Input(value=self.initial_price, placeholder="Counter/base limit price", id="price")
            yield Static("", id="form-error")
            with Horizontal(id="actions"):
                yield Button(Text("Cancel [Esc]"), id="cancel")
                yield Button("Review", id="review", variant="primary")

    def action_cancel(self) -> None:
        self.dismiss(None)

    def on_button_pressed(self, event: Button.Pressed) -> None:
        if event.button.id == "cancel":
            self.action_cancel()
            return
        if event.button.id != "review":
            return
        amount = self.query_one("#amount", Input).value.strip()
        price = self.query_one("#price", Input).value.strip()
        error = self.query_one("#form-error", Static)
        try:
            parsed_amount = Decimal(amount)
            parsed_price = Decimal(price)
        except InvalidOperation:
            error.update("Amount and price must be decimal numbers.")
            return
        if not parsed_amount.is_finite() or parsed_amount <= 0:
            error.update("Amount must be greater than zero.")
            return
        if not parsed_price.is_finite() or parsed_price <= 0:
            error.update("Price must be greater than zero.")
            return
        self.dismiss(
            DexOfferAction(
                kind="update" if self.offer is not None else "create",
                pair=self.pair,
                side=self.side,
                amount=amount,
                price=price,
                offer=self.offer,
            )
        )


class OfferReviewDialog(ModalScreen[bool]):
    BINDINGS = [("escape", "cancel", "Cancel")]

    CSS = """
    OfferReviewDialog { align: center middle; }
    OfferReviewDialog > #dialog { width: 94; height: auto; max-height: 90%; padding: 1 2; border: round $warning; background: $surface; }
    OfferReviewDialog #review-text { margin: 1 0; }
    OfferReviewDialog #actions { height: auto; align-horizontal: right; }
    OfferReviewDialog Button { margin-left: 1; }
    """

    def __init__(self, review):
        super().__init__()
        self.review = review

    def compose(self) -> ComposeResult:
        if self.review.action == "cancel":
            text = (
                f"Cancel offer: #{self.review.offer_id}\n"
                f"Canonical assets: {self.review.base_asset} -> {self.review.counter_asset}\n"
                f"Fee: {self.review.fee} XLM\n"
                f"Network: {self.review.network}"
            )
        else:
            trustline = (
                f"\nAlso create trustline: {self.review.trustline_asset}"
                if self.review.trustline_asset
                else ""
            )
            text = (
                f"Action: {self.review.action.upper()} {self.review.side.upper()}\n"
                f"Pair: {self.review.base_asset} / {self.review.counter_asset}\n"
                f"Amount: {self.review.amount} {self.review.base_asset}\n"
                f"Price: {self.review.price} {self.review.counter_asset}/{self.review.base_asset}\n"
                f"Total: {self.review.total} {self.review.counter_asset}"
                f"{trustline}\n"
                f"Fee: {self.review.fee} XLM\n"
                f"Network: {self.review.network}"
            )
        with Vertical(id="dialog"):
            yield Label("Confirm Stellar DEX operation")
            yield Static(text, id="review-text")
            with Horizontal(id="actions"):
                yield Button(Text("Cancel [Esc]"), id="cancel")
                yield Button("Confirm", id="confirm", variant="warning")

    def action_cancel(self) -> None:
        self.dismiss(False)

    def on_button_pressed(self, event: Button.Pressed) -> None:
        self.dismiss(event.button.id == "confirm")


class DexScreen(ModalScreen[None]):
    BINDINGS = [
        Binding("escape", "close", "Back"),
        Binding("r", "refresh", "Refresh"),
        Binding("u", "toggle_timezone", "UTC / Local"),
        Binding("f", "favorite_market", "Star / Unstar"),
        Binding("w", "swap_pair", "Swap pair"),
        Binding("b", "buy", "Buy"),
        Binding("s", "sell", "Sell"),
        Binding("e", "edit", "Edit offer"),
        Binding("x", "cancel_offer", "Cancel offer"),
    ]

    CSS = """
    DexScreen { layout: vertical; width: 100%; height: 100%; background: $background; opacity: 100%; padding: 1 2; }
    #dex-title { height: auto; text-style: bold; }
    #dex-assets { height: auto; color: $text-muted; }
    #dex-actions { height: auto; margin: 1 0; }
    #dex-actions Button { margin-right: 1; }
    #dex-status { height: 1; color: $text-muted; margin-bottom: 1; }
    .dex-section { height: 1; text-style: bold; }
    #book-row { height: 2fr; min-height: 8; }
    .book-pane { width: 1fr; height: 1fr; padding: 0 1; }
    #bids-pane { align-horizontal: right; }
    .bid-section { text-align: right; }
    #dex-bids { width: auto; min-width: 34; }
    #dex-asks, #dex-bids { height: 1fr; }
    #dex-trades { height: 1fr; min-height: 6; }
    #account-row { height: 2fr; min-height: 8; }
    #offers-pane, #fills-pane { width: 1fr; height: 1fr; padding: 0 1; }
    #dex-offers, #dex-fills { height: 1fr; }
    """

    def __init__(self, runtime, pair: MarketPair, on_offer_action):
        super().__init__()
        self.runtime = runtime
        self.pair = pair
        self.on_offer_action = on_offer_action
        self._visible_offers: list[OpenOffer] = []
        self._visible_fills = []
        self._recent_trades: list[dict] = []
        self._orderbook: dict = {}
        self._refresh_revision = 0
        self._stream_revision = 0
        self._stream_active = False
        self._streams_started = False
        self._orderbook_live = False
        self._trades_live = False
        self._counts = (0, 0, 0, 0, 0)

    def compose(self) -> ComposeResult:
        yield Static("", id="dex-title")
        yield Static("", id="dex-assets")
        with Horizontal(id="dex-actions"):
            yield Button(Text("Buy [B]"), id="buy", variant="primary")
            yield Button(Text("Sell [S]"), id="sell")
            yield Button(Text("⇄ Swap [W]"), id="swap")
            yield Button(Text("★ Star [F]"), id="favorite")
        yield Static("Loading market snapshot...", id="dex-status")
        with Horizontal(id="book-row"):
            with Vertical(id="bids-pane", classes="book-pane"):
                yield Label("BID · BUY", classes="dex-section bid-section")
                yield DataTable(id="dex-bids")
            with Vertical(id="asks-pane", classes="book-pane"):
                yield Label("ASK · SELL", classes="dex-section")
                yield DataTable(id="dex-asks")
        yield Label("Recent market trades · realtime", classes="dex-section")
        yield DataTable(id="dex-trades")
        with Horizontal(id="account-row"):
            with Vertical(id="offers-pane"):
                yield Label("Your open offers", classes="dex-section")
                yield DataTable(id="dex-offers")
            with Vertical(id="fills-pane"):
                yield Label("Your fills", classes="dex-section")
                yield DataTable(id="dex-fills")
        yield Footer()

    def on_mount(self) -> None:
        bids = self.query_one("#dex-bids", DataTable)
        bids.add_columns("Amount", "Price")
        bids.cursor_type = "row"
        asks = self.query_one("#dex-asks", DataTable)
        asks.add_columns("Price", "Amount")
        asks.cursor_type = "row"
        trades = self.query_one("#dex-trades", DataTable)
        trades.add_columns("Price", "Amount", self._time_column())
        trades.cursor_type = "row"
        offers = self.query_one("#dex-offers", DataTable)
        offers.add_columns("Side", "Amount", "Price", "Total", "Offer ID")
        offers.cursor_type = "row"
        fills = self.query_one("#dex-fills", DataTable)
        fills.add_columns(self._time_column(), "Side", "Amount", "Price", "Total", "Fills", "Offer")
        fills.cursor_type = "row"
        self.call_later(self._update_pair_labels)
        self.refresh_market()

    def on_unmount(self) -> None:
        self._stop_realtime()

    def on_button_pressed(self, event: Button.Pressed) -> None:
        actions = {
            "buy": self.action_buy,
            "sell": self.action_sell,
            "swap": self.action_swap_pair,
            "favorite": self.action_favorite_market,
        }
        action = actions.get(event.button.id)
        if action is not None:
            action()

    def action_close(self) -> None:
        self.dismiss(None)

    def action_refresh(self) -> None:
        self.refresh_market()

    def action_toggle_timezone(self) -> None:
        settings = getattr(self.runtime, "settings", None)
        if settings is None:
            self.set_status("Timezone preference is unavailable in this runtime.")
            return
        settings.use_local_time = not bool(getattr(settings, "use_local_time", True))
        store = getattr(self.runtime, "settings_store", None)
        if store is not None:
            store.save(settings)
        self._update_time_columns()
        self._render_recent_trades()
        self._render_fills()
        zone = "local" if settings.use_local_time else "UTC"
        suffix = self._realtime_label() if self._streams_started else "snapshot loaded"
        self._set_market_status(f"{suffix} · {zone} time")

    def action_favorite_market(self) -> None:
        store = getattr(self.runtime, "market_preferences", None)
        if store is None:
            self.set_status("Market favorites are unavailable in this runtime.")
            return
        try:
            session = self.runtime.wallet_manager.view()
            preferences = store.toggle_favorite(
                session.record.network,
                session.wallet.address(),
                self.pair,
            )
        except (FresnicaError, ValueError) as exc:
            self.set_status(str(exc))
            return
        self._update_title(self.pair in preferences.favorites)

    def action_swap_pair(self) -> None:
        swapped = MarketPair(self.pair.counter, self.pair.base)
        try:
            session = self.runtime.wallet_manager.view()
            store = getattr(self.runtime, "market_preferences", None)
            if store is not None:
                store.touch(session.record.network, session.wallet.address(), swapped)
        except (FresnicaError, ValueError):
            pass
        self._stop_realtime()
        self.pair = swapped
        self._visible_offers = []
        self._visible_fills = []
        self._recent_trades = []
        self._orderbook = {}
        self._counts = (0, 0, 0, 0, 0)
        self._clear_market_tables()
        self._update_pair_labels()
        self.set_status("Pair swapped · refreshing market orientation...")
        self.refresh_market()

    def action_buy(self) -> None:
        self._open_offer_form("buy")

    def action_sell(self) -> None:
        self._open_offer_form("sell")

    def action_edit(self) -> None:
        offer = self._selected_offer()
        if offer is None:
            self.set_status("Select an open offer before editing.")
            return
        view = offer_view_for_pair(offer, self.pair)
        if view is None:
            self.set_status("Selected offer is outside this market.")
            return
        self._open_offer_form(view.side, offer)

    def action_cancel_offer(self) -> None:
        offer = self._selected_offer()
        if offer is None:
            self.set_status("Select an open offer before cancelling.")
            return
        self.on_offer_action(
            self,
            DexOfferAction(kind="cancel", pair=self.pair, offer=offer),
        )

    def _open_offer_form(
        self,
        side: Literal["buy", "sell"],
        offer: OpenOffer | None = None,
    ) -> None:
        self.app.push_screen(
            OfferFormDialog(self.pair, side, offer),
            lambda action: self._on_offer_form(action),
        )

    def _on_offer_form(self, action: DexOfferAction | None) -> None:
        if action is not None:
            self.on_offer_action(self, action)

    def _selected_offer(self) -> OpenOffer | None:
        if not self._visible_offers:
            return None
        table = self.query_one("#dex-offers", DataTable)
        index = max(0, min(table.cursor_row, len(self._visible_offers) - 1))
        return self._visible_offers[index]

    def refresh_market(self) -> None:
        self._refresh_revision += 1
        revision = self._refresh_revision
        pair = self.pair
        self.set_status("Refreshing order book, recent trades, offers, and fills...")
        self._refresh_market(revision, pair)

    @work(exclusive=True, thread=True, exit_on_error=False)
    def _refresh_market(self, revision: int, pair: MarketPair) -> None:
        try:
            session = self.runtime.wallet_manager.view()
            services = self.runtime.services_for(session.record.network)
            orderbook = services.dex_service.get_orderbook(pair.base, pair.counter)
            get_trades = getattr(services.dex_service, "get_trades", None)
            recent_trades = (
                get_trades(pair.base, pair.counter, limit=30, refresh=True)
                if get_trades is not None
                else []
            )
            offers = services.dex_service.get_open_offers(
                session.wallet,
                limit=200,
                refresh=True,
            )
            offer_rows = []
            for offer in offers:
                view = offer_view_for_pair(offer, pair)
                if view is not None:
                    offer_rows.append((offer, view))
            segments = services.dex_service.get_account_trade_segments(
                session.wallet,
                limit=1000,
                refresh=True,
            )
            fills = [
                projected
                for segment in segments
                if (projected := account_trade_segment_for_pair(segment, pair)) is not None
            ]
            self.app.call_from_thread(
                self._apply_market,
                revision,
                pair,
                orderbook,
                offer_rows,
                fills,
                recent_trades,
                None,
            )
        except (FresnicaError, ValueError) as exc:
            self.app.call_from_thread(
                self._apply_market,
                revision,
                pair,
                {},
                [],
                [],
                [],
                exc,
            )

    def _apply_market(
        self,
        revision,
        pair,
        orderbook,
        offer_rows,
        fills,
        recent_trades,
        error,
    ) -> None:
        if (
            not self.is_mounted
            or revision != self._refresh_revision
            or pair != self.pair
        ):
            return
        self._orderbook = orderbook
        self._recent_trades = _dedupe_trades(recent_trades)[:30]
        self._render_orderbook(orderbook)
        self._render_recent_trades()

        offers = self.query_one("#dex-offers", DataTable)
        offers.clear()
        self._visible_offers = [offer for offer, _ in offer_rows]
        for offer, view in offer_rows:
            offers.add_row(
                view.side.upper(),
                format_amount(view.amount),
                _stellar_decimal_text(view.price),
                format_amount(view.total),
                offer.offer_id,
            )

        self._visible_fills = list(fills)
        self._render_fills()

        self._counts = (
            len(orderbook.get("asks", [])),
            len(orderbook.get("bids", [])),
            len(self._recent_trades),
            len(offer_rows),
            len(fills),
        )
        if error is not None:
            details = getattr(error, "details", None)
            text = f"ERROR {error}"
            if details:
                text += f" · DEV {details}"
            self.set_status(text)
            return
        self._set_market_status("snapshot loaded · realtime connecting")
        if not self._streams_started:
            self._start_realtime(pair)

    def _render_orderbook(self, orderbook: dict) -> None:
        asks = self.query_one("#dex-asks", DataTable)
        bids = self.query_one("#dex-bids", DataTable)
        asks.clear()
        bids.clear()
        for row in orderbook.get("asks", []):
            amount = _decimal(row.get("amount", "0"))
            asks.add_row(
                _stellar_decimal_text(_book_price(row), style="red"),
                _stellar_decimal_text(amount),
            )
        for row in orderbook.get("bids", []):
            amount = _bid_base_amount(row)
            bids.add_row(
                _stellar_decimal_text(amount, justify="right"),
                _stellar_decimal_text(_book_price(row), style="green", justify="right"),
            )

    def _render_recent_trades(self) -> None:
        table = self.query_one("#dex-trades", DataTable)
        table.clear()
        for raw in self._recent_trades[:30]:
            buy = not bool(raw.get("base_is_seller"))
            table.add_row(
                _stellar_decimal_text(_trade_price(raw), style="green" if buy else "red"),
                format_amount(_decimal(raw.get("base_amount", "0"))),
                self._time(raw.get("ledger_close_time")),
            )

    def _render_fills(self) -> None:
        table = self.query_one("#dex-fills", DataTable)
        table.clear()
        for item in self._visible_fills:
            table.add_row(
                self._time(item.last_time or item.first_time),
                item.side.upper(),
                format_amount(item.base_amount),
                _stellar_ratio_text(item.price_r),
                format_amount(item.counter_amount),
                str(item.trade_count),
                offer_id_label(item.user_offer_id),
            )

    def _start_realtime(self, pair: MarketPair) -> None:
        try:
            session = self.runtime.wallet_manager.view()
            services = self.runtime.services_for(session.record.network)
        except (FresnicaError, ValueError):
            return
        adapter = getattr(services, "adapter", None)
        if adapter is None:
            self._set_market_status("snapshot only · realtime unavailable")
            return
        orderbook_stream = getattr(adapter, "stream_orderbook", None)
        trade_stream = getattr(adapter, "stream_trades", None)
        if orderbook_stream is None and trade_stream is None:
            self._set_market_status("snapshot only · realtime unavailable")
            return
        self._streams_started = True
        self._stream_active = True
        self._stream_revision += 1
        revision = self._stream_revision
        if orderbook_stream is not None:
            self._stream_orderbook(revision, pair)
        if trade_stream is not None:
            cursor = _paging_token(self._recent_trades[0]) if self._recent_trades else "now"
            self._stream_trades(revision, pair, cursor)

    def _stop_realtime(self) -> None:
        self._stream_active = False
        self._stream_revision += 1
        self._streams_started = False
        self._orderbook_live = False
        self._trades_live = False

    @work(thread=True, exit_on_error=False)
    def _stream_orderbook(self, revision: int, pair: MarketPair) -> None:
        try:
            session = self.runtime.wallet_manager.view()
            adapter = self.runtime.services_for(session.record.network).adapter
            for snapshot in adapter.stream_orderbook(pair.base, pair.counter):
                if not self._stream_current(revision, pair):
                    return
                self.app.call_from_thread(
                    self._apply_orderbook_stream,
                    revision,
                    pair,
                    snapshot,
                )
        except (FresnicaError, ValueError) as exc:
            if self._stream_current(revision, pair):
                self.app.call_from_thread(self._stream_failed, revision, pair, "order book", exc)

    @work(thread=True, exit_on_error=False)
    def _stream_trades(
        self,
        revision: int,
        pair: MarketPair,
        cursor: str | None,
    ) -> None:
        try:
            session = self.runtime.wallet_manager.view()
            adapter = self.runtime.services_for(session.record.network).adapter
            for trade in adapter.stream_trades(pair.base, pair.counter, cursor=cursor):
                if not self._stream_current(revision, pair):
                    return
                self.app.call_from_thread(
                    self._apply_trade_stream,
                    revision,
                    pair,
                    trade,
                )
        except (FresnicaError, ValueError) as exc:
            if self._stream_current(revision, pair):
                self.app.call_from_thread(self._stream_failed, revision, pair, "trades", exc)

    def _apply_orderbook_stream(self, revision: int, pair: MarketPair, snapshot: dict) -> None:
        if not self._stream_current(revision, pair) or not self.is_mounted:
            return
        self._orderbook = snapshot
        self._render_orderbook(snapshot)
        self._orderbook_live = True
        self._counts = (
            len(snapshot.get("asks", [])),
            len(snapshot.get("bids", [])),
            self._counts[2],
            self._counts[3],
            self._counts[4],
        )
        self._set_market_status(self._realtime_label())

    def _apply_trade_stream(self, revision: int, pair: MarketPair, trade: dict) -> None:
        if not self._stream_current(revision, pair) or not self.is_mounted:
            return
        key = _trade_key(trade)
        if key is not None:
            self._recent_trades = [
                item for item in self._recent_trades if _trade_key(item) != key
            ]
        self._recent_trades.insert(0, trade)
        self._recent_trades = _dedupe_trades(self._recent_trades)[:30]
        self._render_recent_trades()
        self._trades_live = True
        self._counts = (
            self._counts[0],
            self._counts[1],
            len(self._recent_trades),
            self._counts[3],
            self._counts[4],
        )
        self._set_market_status(self._realtime_label())

    def _stream_failed(self, revision: int, pair: MarketPair, name: str, error) -> None:
        if not self._stream_current(revision, pair) or not self.is_mounted:
            return
        self._set_market_status(
            f"realtime {name} disconnected · R keeps REST snapshot available"
        )

    def _stream_current(self, revision: int, pair: MarketPair) -> bool:
        return (
            self._stream_active
            and revision == self._stream_revision
            and pair == self.pair
        )

    def _realtime_label(self) -> str:
        if self._orderbook_live and self._trades_live:
            return "● realtime order book + trades"
        if self._orderbook_live:
            return "● realtime order book · trades connecting"
        if self._trades_live:
            return "● realtime trades · order book connecting"
        return "realtime connecting"

    def _set_market_status(self, suffix: str) -> None:
        asks, bids, trades, offers, fills = self._counts
        self.set_status(
            f"{asks} asks · {bids} bids · {trades} trades · "
            f"{offers} open offers · {fills} fill segments · {suffix}"
        )

    def _update_pair_labels(self) -> None:
        if not self.is_mounted:
            return
        self.query_one("#dex-assets", Static).update(
            f"BASE  {_asset_identity(self.pair.base)}\n"
            f"COUNTER  {_asset_identity(self.pair.counter)} · "
            f"Price = {_asset_code(self.pair.counter)}/{_asset_code(self.pair.base)}"
        )
        self._update_title()

    def _update_title(self, favorite: bool | None = None) -> None:
        if not self.is_mounted:
            return
        if favorite is None:
            favorite = False
            store = getattr(self.runtime, "market_preferences", None)
            if store is not None:
                try:
                    session = self.runtime.wallet_manager.view()
                    favorite = self.pair in store.get(
                        session.record.network,
                        session.wallet.address(),
                    ).favorites
                except (FresnicaError, ValueError):
                    favorite = False
        star = "★ " if favorite else ""
        self.query_one("#dex-title", Static).update(
            f"{star}Stellar DEX · {_pair_label(self.pair)}"
        )

    def _clear_market_tables(self) -> None:
        for selector in (
            "#dex-asks",
            "#dex-bids",
            "#dex-trades",
            "#dex-offers",
            "#dex-fills",
        ):
            if self.query(selector):
                self.query_one(selector, DataTable).clear()

    def _time(self, value: str | None) -> str:
        settings = getattr(self.runtime, "settings", None)
        return format_timestamp(value, local=bool(getattr(settings, "use_local_time", True)))

    def _time_column(self) -> str:
        settings = getattr(self.runtime, "settings", None)
        use_local = bool(getattr(settings, "use_local_time", True))
        return "Time (local)" if use_local else "Time (UTC)"

    def _update_time_columns(self) -> None:
        label = self._time_column()
        trades = self.query_one("#dex-trades", DataTable)
        trade_columns = list(trades.columns.values())
        if trade_columns:
            trade_columns[-1].label = Text(label)
            trades.refresh()
        fills = self.query_one("#dex-fills", DataTable)
        fill_columns = list(fills.columns.values())
        if fill_columns:
            fill_columns[0].label = Text(label)
            fills.refresh()

    def set_status(self, message: str) -> None:
        if self.is_mounted:
            self.query_one("#dex-status", Static).update(message)


def _asset_identity(asset: Asset) -> str:
    if asset.is_native:
        return "XLM"
    return f"{asset.code}:{asset.issuer}"


def _asset_code(asset: Asset) -> str:
    return "XLM" if asset.is_native else asset.code


def _pair_label(pair: MarketPair) -> str:
    return f"{_asset_code(pair.base)}/{_asset_code(pair.counter)}"


def _decimal(value) -> Decimal:
    try:
        return Decimal(str(value))
    except (InvalidOperation, TypeError, ValueError):
        return Decimal("0")


def _book_price(row: dict) -> Decimal:
    ratio = row.get("price_r") or {}
    if isinstance(ratio, dict):
        n = _decimal(ratio.get("n", "0"))
        d = _decimal(ratio.get("d", "0"))
        if n > 0 and d > 0:
            with localcontext() as context:
                context.prec = 40
                return n / d
    return _decimal(row.get("price", "0"))


def _bid_base_amount(row: dict) -> Decimal:
    amount = _decimal(row.get("amount", "0"))
    ratio = row.get("price_r") or {}
    n = _decimal(ratio.get("n", "0")) if isinstance(ratio, dict) else Decimal("0")
    d = _decimal(ratio.get("d", "0")) if isinstance(ratio, dict) else Decimal("0")
    if n > 0 and d > 0:
        with localcontext() as context:
            context.prec = 40
            return amount * d / n
    price = _decimal(row.get("price", "0"))
    return amount / price if price > 0 else Decimal("0")


def _trade_price(raw: dict) -> Decimal:
    price = raw.get("price") or {}
    if isinstance(price, dict):
        n = _decimal(price.get("n", "0"))
        d = _decimal(price.get("d", "0"))
        if n > 0 and d > 0:
            with localcontext() as context:
                context.prec = 40
                return n / d
    base = _decimal(raw.get("base_amount", "0"))
    counter = _decimal(raw.get("counter_amount", "0"))
    return counter / base if base > 0 else Decimal("0")


def _stellar_decimal_text(
    value,
    style: str | None = None,
    justify: str | None = None,
) -> Text:
    significant, padding = stellar_decimal_parts(value)
    text = Text(justify=justify)
    text.append(significant, style=style)
    if padding:
        pad_style = f"{style} dim" if style else "dim"
        text.append(padding, style=pad_style)
    return text


def _stellar_ratio_text(price) -> Text:
    significant, padding = stellar_price_ratio_parts(price)
    text = Text(significant)
    if padding:
        text.append(padding, style="dim")
    return text


def _trade_key(raw: dict) -> str | None:
    value = raw.get("id") or raw.get("paging_token")
    return str(value) if value is not None else None


def _dedupe_trades(records) -> list[dict]:
    seen: set[str] = set()
    result: list[dict] = []
    for raw in records:
        key = _trade_key(raw)
        if key is not None:
            if key in seen:
                continue
            seen.add(key)
        result.append(raw)
    return result


def _paging_token(raw: dict) -> str | None:
    value = raw.get("paging_token") or raw.get("id")
    return str(value) if value is not None else None