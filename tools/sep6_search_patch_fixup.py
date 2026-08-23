from pathlib import Path
import re

ROOT = Path(__file__).resolve().parents[1]
path = ROOT / "reference" / "python" / "fresnica" / "tui" / "trustlines.py"
text = path.read_text(encoding="utf-8")
start = text.index("    def _apply_trustlines(self, lines, error, cached: bool = False) -> None:\n")
end = text.index("    def set_status(self, message: str) -> None:\n", start)
replacement = '''    def _apply_trustlines(self, lines, error, cached: bool = False) -> None:\n        self._all_lines = list(lines)\n        self._render_lines()\n        if error is not None:\n            details = getattr(error, "details", None)\n            text = f"ERROR {error}"\n            if details:\n                text += f" · DEV {details}"\n            self.set_status(text)\n            return\n        suffix = " · cached; refreshing..." if cached else ""\n        self.set_status(\n            f"{len(self._visible_lines)}/{len(self._all_lines)} issued assets · / search · A add · X remove{suffix}"\n        )\n\n    def _render_lines(self) -> None:\n        table = self.query_one("#trustlines", DataTable)\n        table.clear()\n        self._visible_lines = [\n            raw\n            for raw in self._all_lines\n            if matches_query(\n                self._search_query,\n                raw.get("asset_code"),\n                raw.get("asset_issuer"),\n                _raw_asset_identity(raw),\n            )\n        ]\n        for raw in self._visible_lines:\n            table.add_row(\n                _raw_asset_identity(raw),\n                format_amount(Decimal(str(raw.get("balance", "0")))),\n                format_amount(Decimal(str(raw.get("limit", "0")))),\n                format_amount(Decimal(str(raw.get("buying_liabilities", "0")))),\n                format_amount(Decimal(str(raw.get("selling_liabilities", "0")))),\n            )\n\n'''
path.write_text(text[:start] + replacement + text[end:], encoding="utf-8")
print("trustline filter fixup applied")
