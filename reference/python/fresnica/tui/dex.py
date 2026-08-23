"""Pair-scoped SDEX presentation for the state-driven Fresnica TUI."""

from dataclasses import dataclass
from decimal import Decimal, InvalidOperation
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
        self.initial_price = format_amount(view.price) if view is not None else ""

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
    #dex-book { height: 2fr; min-height: 7; }
    #dex-offers { height: 2fr; min-height: 7; }
    #dex-fills { height: 2fr; min-height: 7; }
    """

    def __init__(self, runtime, pair: MarketPair, on_offer_action):
        super().__init__()
        self.runtime = runtime
        self.pair = pair
        self.on_offer_action = on_offer_action
        self._visible_offers: list[OpenOffer] = []

    def compose(self) -> ComposeResult:
        yield Static(
            f"Stellar DEX · {_asset_code(self.pair.base)}/{_asset_code(self.pair.counter)}",
            id="dex-title",
        )
        yield Static(
            f"BASE  {_asset_identity(self.pair.base)}\nCOUNTER  {_asset_identity(self.pair.counter)}",
            id="dex-assets",
        )
        yield Static("Loading market...", id="dex-status")
        yield Label("Order book", classes="dex-section")
        yield DataTable(id="dex-book")
        yield Label("Your open offers", classes="dex-section")
        yield DataTable(id="dex-offers")
        yield Label("Your fills", classes="dex-section")
        yield DataTable(id="dex-fills")
        yield Footer()

    def on_mount(self) -> None:
        book = self.query_one("#dex-book", DataTable)
        book.add_columns("Side", "Price", "Amount")
        book.cursor_type = "row"
        offers = self.query_one("#dex-offers", DataTable)
        offers.add_columns("Side", "Amount", "Price", "Total", "Offer ID")
        offers.cursor_type = "row"
        fills = self.query_one("#dex-fills", DataTable)
        fills.add_columns("Time", "Side", "Amount", "Price", "Total", "Fills", "Offer ID")
        fills.cursor_type = "row"
        self.refresh_market()

    def action_close(self) -> None:
        self.dismiss(None)

    def action_refresh(self) -> None:
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
        self.set_status("Refreshing order book, offers, and fills...")
        self._refresh_market()

    @work(exclusive=True, thread=True, exit_on_error=False)
    def _refresh_market(self) -> None:
        try:
            session = self.runtime.wallet_manager.view()
            services = self.runtime.services_for(session.record.network)
            orderbook = services.dex_service.get_orderbook(self.pair.base, self.pair.counter)
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
                None,
            )
        except (FresnicaError, ValueError) as exc:
            self.app.call_from_thread(self._apply_market, {}, [], [], exc)

    def _apply_market(self, orderbook, offer_rows, fills, error) -> None:
        book = self.query_one("#dex-book", DataTable)
        book.clear()
        for row in orderbook.get("asks", []):
            book.add_row("ASK", str(row.get("price", "?")), str(row.get("amount", "?")))
        for row in orderbook.get("bids", []):
            book.add_row("BID", str(row.get("price", "?")), str(row.get("amount", "?")))

        offers = self.query_one("#dex-offers", DataTable)
        offers.clear()
        self._visible_offers = [offer for offer, _ in offer_rows]
        for offer, view in offer_rows:
            offers.add_row(
                view.side.upper(),
                format_amount(view.amount),
                format_amount(view.price),
                format_amount(view.total),
                offer.offer_id,
            )

        fill_table = self.query_one("#dex-fills", DataTable)
        fill_table.clear()
        for item in fills:
            price = Decimal(item.price_r.n) / Decimal(item.price_r.d)
            fill_table.add_row(
                format_timestamp(item.last_time or item.first_time),
                item.side.upper(),
                format_amount(item.base_amount),
                format_amount(price),
                format_amount(item.counter_amount),
                str(item.trade_count),
                item.user_offer_id or "-",
            )

        if error is not None:
            details = getattr(error, "details", None)
            text = f"ERROR {error}"
            if details:
                text += f" · DEV {details}"
            self.set_status(text)
            return
        self.set_status(
            f"{len(orderbook.get('asks', []))} asks · {len(orderbook.get('bids', []))} bids · "
            f"{len(offer_rows)} open offers · {len(fills)} fill segments"
        )

    def set_status(self, message: str) -> None:
        if self.is_mounted:
            self.query_one("#dex-status", Static).update(message)


def _asset_identity(asset: Asset) -> str:
    if asset.is_native:
        return "XLM"
    return f"{asset.code}:{asset.issuer}"


def _asset_code(asset: Asset) -> str:
    return "XLM" if asset.is_native else asset.code
