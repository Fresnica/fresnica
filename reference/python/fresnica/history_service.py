"""Recent account operations with raw-data preservation and UI summaries."""

from .models import OperationView


class HistoryService:
    def __init__(self, adapter, datastore, network_name: str):
        self.adapter = adapter
        self.datastore = datastore
        self.network_name = network_name

    def get_operations(self, wallet, limit: int = 20, refresh: bool = True) -> list[dict]:
        address = wallet.address()
        if refresh:
            response = self.adapter.get_operations(address, limit=limit)
            self.datastore.save_operations(self.network_name, address, response)
            return list(response.get("_embedded", {}).get("records", []))
        return self.datastore.get_operations(self.network_name, address, limit=limit)

    def get_views(self, wallet, limit: int = 20, refresh: bool = True) -> list[OperationView]:
        return [self._view(item) for item in self.get_operations(wallet, limit, refresh)]

    @staticmethod
    def _view(raw: dict) -> OperationView:
        operation_type = raw.get("type", "unknown")
        return OperationView(
            operation_type=operation_type,
            created_at=raw.get("created_at"),
            summary=_summary(raw),
            raw=raw,
        )


def _summary(raw: dict) -> str:
    operation_type = raw.get("type")
    if operation_type == "payment":
        code = raw.get("asset_code") or "XLM"
        amount = raw.get("amount", "?")
        source = raw.get("from", "?")
        destination = raw.get("to", "?")
        return f"{amount} {code}  {source} -> {destination}"
    if operation_type == "create_account":
        return (
            f"create {raw.get('account', '?')} with "
            f"{raw.get('starting_balance', '?')} XLM"
        )
    if operation_type in {
        "manage_sell_offer",
        "manage_buy_offer",
        "create_passive_sell_offer",
    }:
        amount = raw.get("amount") or raw.get("buy_amount") or "?"
        price = raw.get("price", "?")
        return f"{operation_type}  amount={amount} price={price}"
    return operation_type or "unknown"
