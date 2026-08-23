"""Rich presentation for Fresnica command mode."""

from datetime import datetime, timezone
from decimal import Decimal, InvalidOperation
import json

from rich.console import Console
from rich.panel import Panel
from rich.prompt import Confirm
from rich.table import Table
from rich.text import Text

from ..presentation import asset_source, format_amount, short_address
from ..review_presentation import project_review


class RichRenderer:
    def __init__(self, console: Console | None = None):
        self.console = console or Console()

    def render_balance(self, record, balances) -> None:
        title = f"{record.name}  {short_address(record.address)}  [{record.network}]"
        table = Table(title=title)
        table.add_column("Asset", style="bold")
        table.add_column("Issuer / source", style="dim")
        table.add_column("Balance", justify="right")
        table.add_column("Available", justify="right", style="bold green")
        table.add_column("In offers", justify="right")
        for item in balances:
            table.add_row(
                item.asset.display,
                asset_source(item.asset),
                format_amount(item.balance),
                format_amount(item.available),
                format_amount(item.selling_liabilities),
            )
        self.console.print(table)

    def render_balance_json(self, record, balances) -> None:
        payload = {
            "wallet": record.name,
            "address": record.address,
            "network": record.network,
            "balances": [
                {
                    "asset": item.asset.display,
                    "issuer": item.asset.issuer,
                    "liquidity_pool_id": item.asset.liquidity_pool_id,
                    "balance": str(item.balance),
                    "selling_liabilities": str(item.selling_liabilities),
                    "buying_liabilities": str(item.buying_liabilities),
                    "available": str(item.available),
                    "raw": item.raw,
                }
                for item in balances
            ],
        }
        self.console.print_json(json.dumps(payload, ensure_ascii=False))

    def render_history(self, record, operations) -> None:
        table = Table(
            title=f"History  {record.name}  {short_address(record.address)}  [{record.network}]"
        )
        table.add_column("Time", style="dim")
        table.add_column("Activity")
        for item in operations:
            table.add_row(item.created_at or "", item.summary)
        self.console.print(table)

    def render_orderbook(self, selling: str, buying: str, orderbook: dict, network: str) -> None:
        table = Table(title=f"Order book  {selling} -> {buying}  [{network}]")
        table.add_column("Side", style="bold")
        table.add_column("Price", justify="right")
        table.add_column("Amount", justify="right")
        for row in orderbook.get("asks", []):
            table.add_row("ASK", str(row.get("price", "?")), str(row.get("amount", "?")))
        for row in orderbook.get("bids", []):
            table.add_row("BID", str(row.get("price", "?")), str(row.get("amount", "?")))
        self.console.print(table)

    def render_offers(self, record, offers: list[dict]) -> None:
        table = Table(title=f"Offers  {record.name}  [{record.network}]")
        table.add_column("ID", style="dim")
        table.add_column("Selling", style="bold")
        table.add_column("Buying", style="bold")
        table.add_column("Amount", justify="right")
        table.add_column("Price", justify="right")
        for offer in offers:
            table.add_row(
                str(offer.get("id", "?")),
                _horizon_asset(offer.get("selling", {})),
                _horizon_asset(offer.get("buying", {})),
                str(offer.get("amount", "?")),
                str(offer.get("price", "?")),
            )
        self.console.print(table)

    def render_account_trade_segments(self, record, segments) -> None:
        table = Table(title=f"Offer fills  {record.name}  [{record.network}]")
        table.add_column("Time", style="dim")
        table.add_column("Side", style="bold")
        table.add_column("Pair")
        table.add_column("Base", justify="right")
        table.add_column("Counter", justify="right")
        table.add_column("Price", justify="right")
        table.add_column("Fills", justify="right")
        table.add_column("Offer", style="dim")
        for item in segments:
            price = Decimal(item.price_r.n) / Decimal(item.price_r.d)
            table.add_row(
                item.last_time or item.first_time or "",
                item.side.upper(),
                f"{item.pair.base.display}/{item.pair.counter.display}",
                format_amount(item.base_amount),
                format_amount(item.counter_amount),
                format_amount(price),
                str(item.trade_count),
                item.user_offer_id or "-",
            )
        self.console.print(table)

    def render_trades(self, base: str, counter: str, trades: list[dict], network: str) -> None:
        table = Table(title=f"Trades  {base} / {counter}  [{network}]")
        table.add_column("Time", style="dim")
        table.add_column("Base", justify="right")
        table.add_column("Counter", justify="right")
        table.add_column("Price", justify="right")
        table.add_column("Base side")
        for trade in trades:
            table.add_row(
                str(trade.get("ledger_close_time", "")),
                str(trade.get("base_amount", "?")),
                str(trade.get("counter_amount", "?")),
                _trade_price(trade),
                "sell" if trade.get("base_is_seller") else "buy",
            )
        self.console.print(table)

    def render_trade_aggregations(
        self,
        base: str,
        counter: str,
        resolution: str,
        aggregations: list[dict],
        network: str,
    ) -> None:
        table = Table(title=f"Candles  {base} / {counter}  {resolution}  [{network}]")
        table.add_column("Time", style="dim")
        table.add_column("Open", justify="right")
        table.add_column("High", justify="right")
        table.add_column("Low", justify="right")
        table.add_column("Close", justify="right")
        table.add_column("Base vol", justify="right")
        table.add_column("Trades", justify="right")
        for item in reversed(aggregations):
            table.add_row(
                _timestamp(item.get("timestamp")),
                str(item.get("open", "?")),
                str(item.get("high", "?")),
                str(item.get("low", "?")),
                str(item.get("close", "?")),
                str(item.get("base_volume", "?")),
                str(item.get("trade_count", "?")),
            )
        self.console.print(table)

    def render_review(self, review) -> None:
        self._render_review_projection(review)

    def render_offer_review(self, review) -> None:
        self._render_review_projection(review)

    def _render_review_projection(self, review) -> None:
        presentation = project_review(review)
        text = Text()
        text.append(presentation.summary, style="bold")
        text.append("\n\n")
        for index, field in enumerate(presentation.fields):
            if index:
                text.append("\n")
            text.append(f"{field.label}: ", style="dim")
            text.append(field.value)
        for warning in presentation.warnings:
            text.append("\n")
            text.append(f"Warning: {warning}", style="bold yellow")
        self.console.print(
            Panel(text, title=presentation.title, border_style="yellow")
        )

    def confirm(self) -> bool:
        return Confirm.ask("Confirm", default=False, console=self.console)

    def render_result(self, result, network) -> None:
        text = Text()
        text.append("Transaction submitted\n", style="bold green")
        text.append("Hash: ")
        text.append(result.hash, style="bold cyan")
        if result.ledger is not None:
            text.append(f"\nLedger: {result.ledger}")
        if result.hash:
            text.append(
                f"\nExplorer: https://stellar.expert/explorer/{network.explorer_network}/tx/{result.hash}",
                style="dim",
            )
        self.console.print(Panel(text, title="Success", border_style="green"))

    def render_wallets(self, records, default_name: str | None) -> None:
        table = Table(title="Wallets")
        table.add_column("")
        table.add_column("Name", style="bold")
        table.add_column("Address")
        table.add_column("Type")
        table.add_column("Network")
        for record in records:
            table.add_row(
                "*" if record.name == default_name else "",
                record.name,
                short_address(record.address),
                record.wallet_type,
                record.network,
            )
        self.console.print(table)

    def render_contacts(self, contacts) -> None:
        table = Table(title="Contacts")
        table.add_column("Name", style="bold")
        table.add_column("Address")
        table.add_column("Default memo")
        for contact in contacts:
            table.add_row(
                contact.name,
                short_address(contact.address),
                contact.memo or "-",
            )
        self.console.print(table)

    def render_info(self, record) -> None:
        text = Text()
        text.append(f"Name: {record.name}\n", style="bold")
        text.append(f"Address: {record.address}\n")
        text.append(f"Network: {record.network}\n")
        text.append(f"Type: {record.wallet_type}\n")
        text.append("Signing: watch-only" if record.watch_only else "Signing: locked")
        self.console.print(Panel(text, title="Wallet"))

    def render_created_mnemonic(self, record, mnemonic: str) -> None:
        text = Text()
        text.append(
            "Back up these words now. They will not be stored in plaintext.\n\n",
            style="bold yellow",
        )
        text.append(mnemonic, style="bold")
        self.console.print(
            Panel(text, title=f"Wallet {record.name} created", border_style="yellow")
        )

    def success(self, message: str) -> None:
        text = Text("OK ", style="bold green")
        text.append(message)
        self.console.print(text)

    def error(self, message: str, details: str | None = None) -> None:
        text = Text("ERROR ", style="bold red")
        text.append(message)
        self.console.print(text)
        if details:
            dev = Text("DEV ", style="bold dim")
            dev.append(details, style="dim")
            self.console.print(dev)


def _horizon_asset(raw: dict) -> str:
    if raw.get("asset_type") == "native":
        return "XLM"
    code = raw.get("asset_code", "?")
    issuer = raw.get("asset_issuer")
    return f"{code}:{short_address(issuer)}" if issuer else code


def _trade_price(raw: dict) -> str:
    price = raw.get("price")
    if isinstance(price, dict) and price.get("d") not in (None, "0", 0):
        try:
            return format_amount(Decimal(str(price["n"])) / Decimal(str(price["d"])))
        except (InvalidOperation, KeyError):
            pass
    try:
        base = Decimal(str(raw.get("base_amount")))
        counter = Decimal(str(raw.get("counter_amount")))
        if base:
            return format_amount(counter / base)
    except (InvalidOperation, TypeError, ValueError):
        pass
    return "?"


def _timestamp(value) -> str:
    try:
        return datetime.fromtimestamp(int(value) / 1000, timezone.utc).strftime("%Y-%m-%d %H:%M")
    except (TypeError, ValueError, OSError):
        return str(value or "")
