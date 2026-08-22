"""Rich presentation for Fresnica command mode."""

import json

from rich.console import Console
from rich.panel import Panel
from rich.prompt import Confirm
from rich.table import Table
from rich.text import Text


class RichRenderer:
    def __init__(self, console: Console | None = None):
        self.console = console or Console()

    def render_balance(self, record, balances) -> None:
        title = f"{record.name}  {short_address(record.address)}  [{record.network}]"
        table = Table(title=title)
        table.add_column("Asset", style="bold")
        table.add_column("Balance", justify="right")
        table.add_column("Liabilities", justify="right")
        table.add_column("Available", justify="right", style="bold green")
        for item in balances:
            table.add_row(
                item.asset.display,
                _decimal(item.balance),
                _decimal(item.selling_liabilities),
                _decimal(item.available),
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
        table.add_column("Type", style="bold")
        table.add_column("Summary")
        for item in operations:
            table.add_row(item.created_at or "", item.operation_type, item.summary)
        self.console.print(table)

    def render_review(self, review) -> None:
        text = Text()
        text.append("You (", style="dim")
        text.append(review.wallet_name, style="bold cyan")
        text.append(f", {short_address(review.source)})\n\n", style="dim")
        text.append("will transfer\n", style="dim")
        text.append(f"{review.amount} {review.asset}", style="bold green")
        text.append("\n\nto\n", style="dim")
        if review.contact_name:
            text.append(review.contact_name + " ", style="bold cyan")
        text.append(review.destination, style="bold yellow")
        text.append(f"\n\nFee: {review.fee} XLM", style="dim")
        text.append(f"\nNetwork: {review.network}", style="dim")
        if review.memo:
            text.append(f"\nMemo: {review.memo}", style="dim")
        self.console.print(Panel(text, title="Confirm transaction", border_style="yellow"))

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

    def error(self, message: str) -> None:
        text = Text("ERROR ", style="bold red")
        text.append(message)
        self.console.print(text)


def short_address(address: str, head: int = 6, tail: int = 4) -> str:
    if len(address) <= head + tail + 3:
        return address
    return f"{address[:head]}...{address[-tail:]}"


def _decimal(value) -> str:
    if value is None:
        return "-"
    text = format(value, "f")
    if "." in text:
        text = text.rstrip("0").rstrip(".")
    return text or "0"
