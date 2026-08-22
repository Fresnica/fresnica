"""Rich based rendering helpers.

Presentation only. Business logic stays in services.
"""

from rich.console import Console
from rich.panel import Panel
from rich.table import Table


class RichRenderer:
    def __init__(self):
        self.console = Console()

    def render_balance(self, balances):
        table = Table(title="Balance")
        table.add_column("Asset")
        table.add_column("Amount")

        for item in balances:
            table.add_row(
                str(item.get("asset_code", "XLM")),
                str(item.get("balance", "0")),
            )

        self.console.print(table)
        return table

    def render_review(self, review):
        panel = Panel(str(review), title="Confirm Transaction")
        self.console.print(panel)
        return panel

    def render_result(self, result):
        panel = Panel(str(result), title="Transaction Submitted")
        self.console.print(panel)
        return panel
