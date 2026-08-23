"""Pair-scoped SDEX presentation for the state-driven Fresnica TUI."""

from dataclasses import dataclass
from decimal import Decimal, InvalidOperation, localcontext
from typing import Literal

from textual import work
from textual.app import ComposeResult
from textual.binding import Binding
from textual.containers import Horizontal, Vertical
from textual.screen import ModalScreen
from textual.widgets import Button, DataTable, Footer, Input, Label, Static

from ..errors import FresnicaError
from ..models import Asset, MarketPair, OpenOffer
from ..offer_service import offer_view_for_pair
from ..presentation import format_amount, format_timestamp
from ..sdex_presentation import format_market_price, format_price_ratio, offer_id_label
from ..trade_segments import account_trade_segment_for_pair


DexActionKind = Literal["create", "update", "cancel"]


@dataclass(frozen=True)
class DexOfferAction:
    kind: DexActionKind
    pair: MarketPair
    side: Literal["buy", "sell"] | None = None
    amount: str | None = None
    price: str | None = None
    offer: OpenOffer | None = None


class MarketPairDialog(ModalScreen[MarketPair | None]):
    BINDINGS = [("escape", "cancel", "Cancel")]

    CSS = """
    MarketPairDialog { align: center middle; }
    MarketPairDialog > #dialog { width: 88; height: auto; padding: 1 2; border: round $accent; background: $surface; }
    MarketPairDialog Input { margin-top: 1; }
    MarketPairDialog #market-help { color: $text-muted; margin-top: 1; }
    MarketPairDialog #form-error { color: $error; margin-top: 1; }
    MarketPairDialog #actions { height: auto; margin-top: 1; align-horizontal: right; }
    MarketPairDialog Button { margin-left: 1; }
    """

    def compose(self) -> ComposeResult:
        with Vertical(id="dialog"):
            yield Label("Open Stellar DEX market")
            yield Input(value="XLM", placeholder="Base asset: XLM or CODE:GISSUER...", id="base")
            yield Input(placeholder="Counter asset: XLM or CODE:GISSUER...", id="counter")
            yield Static(
                "Price is always COUNTER per one BASE. BUY and SELL use BASE amount.",
                id="market-help",
            )
            yield Static("", id="form-error")
            with Horizontal(id="actions"):
                yield Button("Cancel", id="cancel")
                yield Button("Open market", id="open", variant="primary")

    def action_cancel(self) -> None:
        self.dismiss(None)

    def on_button_pressed(self, event: Button.Pressed) -> None:
        if event.button.id == "cancel":
            self.action_cancel()
            return
        if event.button.id != "open":
            return
        error = self.query_one("#form-error", Static)
        try:
            base = Asset.parse(self.query_one("#base", Input).value.strip())
            counter = Asset.parse(self.query_one("#counter", Input).value.strip())
        except (FresnicaError, ValueError) as exc:
            error.update(str(exc))
            return
        if base == counter:
            error.update("Base and counter assets must be different.")
            return
        self.dismiss(MarketPair(base=base, counter=counter))


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
                yield Button("Cancel", id="cancel")
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
                yield Button("Cancel", id="cancel")
                yield Button("Confirm", id="confirm", variant="warning")

    def action_cancel(self) -> None:
        self.dismiss(False)

    def on_button_pressed(self, event: Button.Pressed) -> None:
        self.dismiss(event.button.id == "confirm")


class DexScreen(ModalScreen[None]):
    BINDINGS = [
        Binding("escape", "close", "Back"),
        Binding("r", "refresh", "Refresh"),
        Binding("f", "favorite_market", "Star / Unstar"),
        Binding("b", "buy", "Buy"),
        Binding("s", "sell", "Sell"),
        Binding("e", "edit", "Edit offer"),
        Binding("x", "cancel_offer", "Cancel offer"),
    ]

    CSS = """
    DexScreen { layout: vertical; background: $surface; padding: 1 2; }
    #dex-title { height: auto; text-style: bold; }
    #dex-assets { height: auto; color: $text-muted; }
    #dex-status { height: 1; color: $text-muted; margin-bottom: 1; }
    .dex-section { height: 1; text-style: bold; }
    #book-row { height: 2fr; min-height: 8; }
    .book-pane { width: 1fr; height: 1fr; padding: 0 1; }
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
        self._recent_trades: list[dict] = []
        self._orderbook: dict = {}
        self._stream_revision = 0
        self._stream_active = False
        self._streams_started = False
        self._orderbook_live = False
        self._trades_live = False
        self._counts = (0, 0, 0, 0, 0)

    def compose(self) -> ComposeResult:
        yield Static("", id="dex-title")
        yield Static(
            f"BASE  {_asset_identity(self.pair.base)}\nCOUNTER  {_asset_identity(self.pair.counter)}",
            id="dex-assets",
        )
        yield Static("Loading market snapshot...", id="dex-status")
        with Horizontal(id="book-row"):
            with Vertical(classes="book-pane"):
                yield Label("ASK · SELL", classes="dex-section")
                yield DataTable(id="dex-asks")
            with Vertical(classes="book-pane"):
                yield Label("BID · BUY", classes="dex-section")
                yield DataTable(id="dex-bids")
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
        for table_id in ("#dex-asks", "#dex-bids"):
            table = self.query_one(table_id, DataTable)
            table.add_columns("Price", "Amount", "Total")
            table.cursor_type = "row"
        trades = self.query_one("#dex-trades", DataTable)
        trades.add_columns("Time", "Side", "Amount", "Price", "Total")
        trades.cursor_type = "row"
        offers = self.query_one("#dex-offers", DataTable)
        offers.add_columns("Side", "Amount", "Price", "Total", "Offer ID")
        offers.cursor_type = "row"
        fills = self.query_one("#dex-fills", DataTable)
        fills.add_columns("Time", "Side", "Amount", "Price", "Total", "Fills", "Offer")
        fills.cursor_type = "row"
        self._update_title()
        self.refresh_market()

    def on_unmount(self) -> None:
        self._stream_active = False
        self._stream_revision += 1

    def action_close(self) -> None:
        self.dismiss(None)

    def action_refresh(self) -> None:
        self.refresh_market()

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
        self.set_status("Refreshing order book, recent trades, offers, and fills...")
        self._refresh_market()

    @work(exclusive=True, thread=True, exit_on_error=False)
    def _refresh_market(self) -> None:
        try:
            session = self.runtime.wallet_manager.view()
            services = self.runtime.services_for(session.record.network)
            orderbook = services.dex_service.get_orderbook(self.pair.base, self.pair.counter)
            recent_trades = services.dex_service.get_trades(
                self.pair.base,
                self.pair.counter,
                limit=30,
                refresh=True,
            )
            offers = services.dex_service.get_open_offers(
                session.wallet,
                limit=200,
                refresh=True,
            )
            offer_rows = []
            for offer in offers:
                view = offer_view_for_pair(offer, self.pair)
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
                if (projected := account_trade_segment_for_pair(segment, self.pair))
                is not None
            ]
            self.app.call_from_thread(
                self._apply_market,
                orderbook,
                offer_rows,
                fills,
                recent_trades,
                None,
            )
        except (FresnicaError, ValueError) as exc:
            self.app.call_from_thread(self._apply_market, {}, [], [], [], exc)

    def _apply_market(self, orderbook, offer_rows, fills, recent_trades, error) -> None:
        if not self.is_mounted:
            return
        self._orderbook = orderbook
        self._recent_trades = list(recent_trades)
        self._render_orderbook(orderbook)
        self._render_recent_trades()

        offers = self.query_one("#dex-offers", DataTable)
        offers.clear()
        self._visible_offers = [offer for offer, _ in offer_rows]
        for offer, view in offer_rows:
            offers.add_row(
                view.side.upper(),
                format_amount(view.amount),
                format_market_price(view.price),
                format_amount(view.total),
                offer.offer_id,
            )

        fill_table = self.query_one("#dex-fills", DataTable)
        fill_table.clear()
        for item in fills:
            fill_table.add_row(
                self._time(item.last_time or item.first_time),
                item.side.upper(),
                format_amount(item.base_amount),
                format_price_ratio(item.price_r),
                format_amount(item.counter_amount),
                str(item.trade_count),
                offer_id_label(item.user_offer_id),
            )

        self._counts = (
            len(orderbook.get("asks", [])),
            len(orderbook.get("bids", [])),
            len(recent_trades),
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
            self._start_realtime()

    def _render_orderbook(self, orderbook: dict) -> None:
        asks = self.query_one("#dex-asks", DataTable)
        bids = self.query_one("#dex-bids", DataTable)
        asks.clear()
        bids.clear()
        for row in orderbook.get("asks", []):
            price = _decimal(row.get("price", "0"))
            amount = _decimal(row.get("amount", "0"))
            asks.add_row(
                format_market_price(price),
                format_amount(amount),
                format_amount(amount * price),
            )
        for row in orderbook.get("bids", []):
            price = _decimal(row.get("price", "0"))
            amount = _bid_base_amount(row)
            bids.add_row(
                format_market_price(price),
                format_amount(amount),
                format_amount(amount * price),
            )

    def _render_recent_trades(self) -> None:
        table = self.query_one("#dex-trades", DataTable)
        table.clear()
        for raw in self._recent_trades[:30]:
            price = _trade_price(raw)
            table.add_row(
                self._time(raw.get("ledger_close_time")),
                "SELL" if raw.get("base_is_seller") else "BUY",
                format_amount(_decimal(raw.get("base_amount", "0"))),
                format_market_price(price),
                format_amount(_decimal(raw.get("counter_amount", "0"))),
            )

    def _start_realtime(self) -> None:
        try:
            session = self.runtime.wallet_manager.view()
            services = self.runtime.services_for(session.record.network)
        except (FresnicaError, ValueError):
            return
        adapter = getattr(services, "adapter", None)
        if adapter is None:
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
            self._stream_orderbook(revision)
        if trade_stream is not None:
            cursor = _paging_token(self._recent_trades[0]) if self._recent_trades else "now"
            self._stream_trades(revision, cursor)

    @work(thread=True, exit_on_error=False)
    def _stream_orderbook(self, revision: int) -> None:
        try:
            session = self.runtime.wallet_manager.view()
            adapter = self.runtime.services_for(session.record.network).adapter
            for snapshot in adapter.stream_orderbook(self.pair.base, self.pair.counter):
                if not self._stream_current(revision):
                    return
                self.app.call_from_thread(self._apply_orderbook_stream, revision, snapshot)
        except (FresnicaError, ValueError) as exc:
            if self._stream_current(revision):
                self.app.call_from_thread(self._stream_failed, revision, "order book", exc)

    @work(thread=True, exit_on_error=False)
    def _stream_trades(self, revision: int, cursor: str | None) -> None:
        try:
            session = self.runtime.wallet_manager.view()
            adapter = self.runtime.services_for(session.record.network).adapter
            for trade in adapter.stream_trades(self.pair.base, self.pair.counter, cursor=cursor):
                if not self._stream_current(revision):
                    return
                self.app.call_from_thread(self._apply_trade_stream, revision, trade)
        except (FresnicaError, ValueError) as exc:
            if self._stream_current(revision):
                self.app.call_from_thread(self._stream_failed, revision, "trades", exc)

    def _apply_orderbook_stream(self, revision: int, snapshot: dict) -> None:
        if not self._stream_current(revision) or not self.is_mounted:
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

    def _apply_trade_stream(self, revision: int, trade: dict) -> None:
        if not self._stream_current(revision) or not self.is_mounted:
            return
        key = _trade_key(trade)
        self._recent_trades = [item for item in self._recent_trades if _trade_key(item) != key]
        self._recent_trades.insert(0, trade)
        del self._recent_trades[30:]
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

    def _stream_failed(self, revision: int, name: str, error) -> None:
        if not self._stream_current(revision) or not self.is_mounted:
            return
        self._set_market_status(f"realtime {name} disconnected · R keeps REST snapshot available")

    def _stream_current(self, revision: int) -> bool:
        return self._stream_active and revision == self._stream_revision

    def _realtime_label(self) -> str:
        if self._orderbook_live and self._trades_live:
            return "realtime order book + trades"
        if self._orderbook_live:
            return "realtime order book · trades connecting"
        if self._trades_live:
            return "realtime trades · order book connecting"
        return "realtime connecting"

    def _set_market_status(self, suffix: str) -> None:
        asks, bids, trades, offers, fills = self._counts
        self.set_status(
            f"{asks} asks · {bids} bids · {trades} recent trades · "
            f"{offers} open offers · {fills} fill segments · {suffix}"
        )

    def _update_title(self, favorite: bool | None = None) -> None:
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
            f"{star}Stellar DEX · {_asset_code(self.pair.base)}/{_asset_code(self.pair.counter)}"
        )

    def _time(self, value: str | None) -> str:
        settings = getattr(self.runtime, "settings", None)
        return format_timestamp(value, local=bool(getattr(settings, "use_local_time", True)))

    def set_status(self, message: str) -> None:
        if self.is_mounted:
            self.query_one("#dex-status", Static).update(message)


def _asset_identity(asset: Asset) -> str:
    if asset.is_native:
        return "XLM"
    return f"{asset.code}:{asset.issuer}"


def _asset_code(asset: Asset) -> str:
    return "XLM" if asset.is_native else asset.code


def _decimal(value) -> Decimal:
    try:
        return Decimal(str(value))
    except (InvalidOperation, TypeError, ValueError):
        return Decimal("0")


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


def _trade_key(raw: dict) -> str:
    return str(raw.get("id") or raw.get("paging_token") or "")


def _paging_token(raw: dict) -> str | None:
    value = raw.get("paging_token") or raw.get("id")
    return str(value) if value is not None else None
