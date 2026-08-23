"""Discoverable market entry for the Textual DEX."""

from dataclasses import dataclass

import requests
from textual import work
from textual.app import ComposeResult
from textual.binding import Binding
from textual.screen import ModalScreen
from textual.widgets import DataTable, Footer, Static

from ..errors import FresnicaError
from ..market_discovery import MarketDiscoveryService
from ..market_preferences import MarketPreferencesStore, pair_identity
from ..models import Asset, MarketPair
from ..presentation import asset_label
from .dex import MarketPairDialog


@dataclass(frozen=True)
class MarketChoice:
    pair: MarketPair
    source: str


class MarketChooserScreen(ModalScreen[MarketPair | None]):
    BINDINGS = [
        Binding("escape", "close", "Back"),
        Binding("enter", "open_market", "Open"),
        Binding("f", "favorite", "Star / Unstar"),
        Binding("a", "add_custom", "Add pair"),
        Binding("r", "refresh_popular", "Popular"),
    ]

    CSS = """
    MarketChooserScreen { layout: vertical; background: $surface; padding: 1 2; }
    #market-title { height: 1; text-style: bold; }
    #market-status { height: 2; color: $text-muted; margin-bottom: 1; }
    #market-list { height: 1fr; }
    """

    def __init__(self, runtime):
        super().__init__()
        self.runtime = runtime
        self._choices: list[MarketChoice] = []
        self._popular: list[MarketPair] = []
        self._scope = None

    def compose(self) -> ComposeResult:
        yield Static("Stellar DEX markets", id="market-title")
        yield Static(
            "Starred and recent markets are local to this wallet. Suggestions use held assets; Popular is best-effort public market data.",
            id="market-status",
        )
        yield DataTable(id="market-list")
        yield Footer()

    def on_mount(self) -> None:
        table = self.query_one("#market-list", DataTable)
        table.add_columns("★", "Pair", "Base", "Counter", "Source")
        table.cursor_type = "row"
        try:
            session = self.runtime.wallet_manager.view()
        except FresnicaError as exc:
            self.query_one("#market-status", Static).update(str(exc))
            return
        self._scope = (session.record.network, session.wallet.address())
        self._render_choices()
        self._load_popular()

    def action_close(self) -> None:
        self.dismiss(None)

    def action_open_market(self) -> None:
        choice = self._selected()
        if choice is None or self._scope is None:
            return
        network, address = self._scope
        self._store().touch(network, address, choice.pair)
        self.dismiss(choice.pair)

    def on_data_table_row_selected(self, event: DataTable.RowSelected) -> None:
        if event.data_table.id == "market-list":
            self.action_open_market()

    def action_favorite(self) -> None:
        choice = self._selected()
        if choice is None or self._scope is None:
            return
        network, address = self._scope
        self._store().toggle_favorite(network, address, choice.pair)
        self._render_choices(
            f"{'Starred' if self._is_favorite(choice.pair) else 'Unstarred'} {_pair_label(choice.pair)}"
        )

    def action_add_custom(self) -> None:
        self.app.push_screen(MarketPairDialog(), self._custom_pair)

    def _custom_pair(self, pair: MarketPair | None) -> None:
        if pair is None or self._scope is None:
            return
        network, address = self._scope
        self._store().touch(network, address, pair)
        self.dismiss(pair)

    def action_refresh_popular(self) -> None:
        self.query_one("#market-status", Static).update("Refreshing popular markets...")
        self._load_popular()

    @work(exclusive=True, thread=True, exit_on_error=False)
    def _load_popular(self) -> None:
        if self._scope is None:
            return
        network, _ = self._scope
        try:
            pairs = MarketDiscoveryService().popular_pairs(network, limit=10)
            self.app.call_from_thread(self._apply_popular, pairs, None)
        except (requests.RequestException, ValueError) as exc:
            self.app.call_from_thread(self._apply_popular, [], exc)

    def _apply_popular(self, pairs: list[MarketPair], error) -> None:
        if not self.is_mounted:
            return
        if error is None:
            self._popular = pairs
            self._render_choices(
                f"{len(pairs)} popular markets loaded · F star · A custom pair"
            )
        else:
            self._render_choices(
                "Popular markets unavailable · starred, recent, held-asset suggestions and custom pair remain available"
            )

    def _render_choices(self, status: str | None = None) -> None:
        if self._scope is None:
            return
        network, address = self._scope
        preferences = self._store().get(network, address)
        choices: list[MarketChoice] = []

        def add(pair: MarketPair, source: str) -> None:
            if pair.base == pair.counter:
                return
            if any(item.pair == pair for item in choices):
                return
            choices.append(MarketChoice(pair, source))

        for pair in preferences.favorites:
            add(pair, "Starred")
        for pair in preferences.recents:
            add(pair, "Recent")
        for pair in self._held_asset_suggestions():
            add(pair, "Held asset")
        for pair in self._popular:
            add(pair, "Popular")

        self._choices = choices
        table = self.query_one("#market-list", DataTable)
        table.clear()
        favorites = set(preferences.favorites)
        for choice in choices:
            pair = choice.pair
            table.add_row(
                "★" if pair in favorites else "",
                _pair_label(pair),
                asset_label(pair.base, include_source=True),
                asset_label(pair.counter, include_source=True),
                choice.source,
            )
        if status is not None:
            self.query_one("#market-status", Static).update(status)
        elif choices:
            self.query_one("#market-status", Static).update(
                f"{len(choices)} markets · Enter open · F star · A custom pair"
            )
        else:
            self.query_one("#market-status", Static).update(
                "No saved markets yet · press A for a custom pair; popular markets are loading"
            )

    def _held_asset_suggestions(self) -> list[MarketPair]:
        try:
            session = self.runtime.wallet_manager.view()
            service = self.runtime.services_for(session.record.network).balance_service
            getter = getattr(service, "get_cached_portfolio_views", None)
            if getter is None:
                return []
            balances, _ = getter(session.wallet)
        except (FresnicaError, ValueError):
            return []
        pairs = []
        for balance in balances:
            asset = balance.asset
            if asset.is_native or asset.is_liquidity_pool or balance.balance <= 0:
                continue
            pairs.append(MarketPair(asset, Asset("XLM")))
        return pairs

    def _selected(self) -> MarketChoice | None:
        if not self._choices:
            return None
        row = self.query_one("#market-list", DataTable).cursor_row
        return self._choices[max(0, min(row, len(self._choices) - 1))]

    def _store(self) -> MarketPreferencesStore:
        store = getattr(self.runtime, "market_preferences", None)
        if store is None:
            raise RuntimeError("Runtime does not provide market_preferences")
        return store

    def _is_favorite(self, pair: MarketPair) -> bool:
        if self._scope is None:
            return False
        network, address = self._scope
        return pair in self._store().get(network, address).favorites


def _pair_label(pair: MarketPair) -> str:
    return f"{pair.base.display}/{pair.counter.display}"
