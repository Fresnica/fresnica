from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
PY = ROOT / "reference" / "python"

# Repair the trustline renderer after the primary transformation and keep
# visible-row state identical to the rendered filtered table.
path = PY / "fresnica" / "tui" / "trustlines.py"
text = path.read_text(encoding="utf-8")
start = text.index("    def _apply_trustlines(self, lines, error, cached: bool = False) -> None:\n")
end = text.index("    def set_status(self, message: str) -> None:\n", start)
replacement = '''    def _apply_trustlines(self, lines, error, cached: bool = False) -> None:\n        self._all_lines = list(lines)\n        self._render_lines()\n        if error is not None:\n            details = getattr(error, "details", None)\n            text = f"ERROR {error}"\n            if details:\n                text += f" · DEV {details}"\n            self.set_status(text)\n            return\n        suffix = " · cached; refreshing..." if cached else ""\n        self.set_status(\n            f"{len(self._visible_lines)}/{len(self._all_lines)} issued assets · / search · A add · X remove{suffix}"\n        )\n\n    def _render_lines(self) -> None:\n        table = self.query_one("#trustlines", DataTable)\n        table.clear()\n        self._visible_lines = [\n            raw\n            for raw in self._all_lines\n            if matches_query(\n                self._search_query,\n                raw.get("asset_code"),\n                raw.get("asset_issuer"),\n                _raw_asset_identity(raw),\n            )\n        ]\n        for raw in self._visible_lines:\n            table.add_row(\n                _raw_asset_identity(raw),\n                format_amount(Decimal(str(raw.get("balance", "0")))),\n                format_amount(Decimal(str(raw.get("limit", "0")))),\n                format_amount(Decimal(str(raw.get("buying_liabilities", "0")))),\n                format_amount(Decimal(str(raw.get("selling_liabilities", "0")))),\n            )\n\n'''
path.write_text(text[:start] + replacement + text[end:], encoding="utf-8")

# The existing cache test only enabled SEP-6 deposit. Add method metadata there
# so the round-trip explicitly covers the new persisted schema.
cache_test = PY / "tests" / "test_anchor_cache.py"
text = cache_test.read_text(encoding="utf-8")
if "sep6_deposit_info=" not in text:
    needle = "        sep6_deposit=True,\n"
    if needle not in text:
        raise SystemExit("anchor cache test insertion point not found")
    text = text.replace(
        needle,
        needle
        + '        sep6_deposit_info={"enabled": True, "fee_fixed": "0"},\n'
        + '        sep6_withdraw_info={"enabled": True, "types": {"crypto": {"fields": {"dest": {}}}}},\n',
        1,
    )
    cache_test.write_text(text, encoding="utf-8")

# Fix the test fake for stellar-sdk v15 Account, whose public account id is not
# exposed as Account.account_id. Keep the asserted source separately.
memo_test = PY / "tests" / "test_anchor_memo_types.py"
text = memo_test.read_text(encoding="utf-8")
text = text.replace(
    '''class Server:\n    def __init__(self, account):\n        self.account = account\n\n    def load_account(self, source):\n        assert source == self.account.account_id\n        return self.account\n''',
    '''class Server:\n    def __init__(self, account, source):\n        self.account = account\n        self.source = source\n\n    def load_account(self, source):\n        assert source == self.source\n        return self.account\n''',
)
text = text.replace(
    '    adapter.server = Server(Account(keypair.public_key, 1))\n',
    '    adapter.server = Server(Account(keypair.public_key, 1), keypair.public_key)\n',
)
memo_test.write_text(text, encoding="utf-8")

# Guard against accidentally ignoring an earlier primary patch failure.
checks = {
    PY / "fresnica" / "anchor_service.py": "class AnchorSep6Transfer",
    PY / "fresnica" / "tui" / "list_search.py": "class ListSearchDialog",
    PY / "fresnica" / "tui" / "asset_details.py": "class Sep6TransferDialog",
    PY / "tests" / "test_anchor_memo_types.py": "adapter.server = Server(Account(keypair.public_key, 1), keypair.public_key)",
}
for target, marker in checks.items():
    if not target.exists() or marker not in target.read_text(encoding="utf-8"):
        raise SystemExit(f"primary patch incomplete: {target} lacks {marker}")

print("SEP-6/search fixups applied")
