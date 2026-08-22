"""Account activity with local caching and human-readable summaries."""

from .models import ActivityView, OperationView
from .presentation import format_amount, short_address


SYNC_PAGE_LIMIT = 200


class HistoryService:
    def __init__(self, adapter, datastore, network_name: str):
        self.adapter = adapter
        self.datastore = datastore
        self.network_name = network_name

    def sync_recent(self, wallet, limit: int = SYNC_PAGE_LIMIT) -> int:
        address = wallet.address()
        cached = self.datastore.get_operations(self.network_name, address, limit=1)
        if cached:
            cursor = _paging_token(cached[0])
            response = self.adapter.get_operations(
                address,
                limit=limit,
                cursor=cursor,
                desc=False,
            )
        else:
            response = self.adapter.get_operations(address, limit=limit, desc=True)
        records = list(response.get("_embedded", {}).get("records", []))
        if records:
            self.datastore.save_operations(self.network_name, address, response)
        return len(records)

    def load_older(self, wallet, limit: int = SYNC_PAGE_LIMIT) -> int:
        address = wallet.address()
        cached = self.datastore.get_operations(self.network_name, address, limit=100000)
        if not cached:
            return self.sync_recent(wallet, limit=limit)
        cursor = _paging_token(cached[-1])
        response = self.adapter.get_operations(
            address,
            limit=limit,
            cursor=cursor,
            desc=True,
        )
        records = list(response.get("_embedded", {}).get("records", []))
        if records:
            self.datastore.save_operations(self.network_name, address, response)
        return len(records)

    def get_operations(self, wallet, limit: int = 20, refresh: bool = True) -> list[dict]:
        address = wallet.address()
        if refresh:
            self.sync_recent(wallet)
        return self.datastore.get_operations(self.network_name, address, limit=limit)

    def get_views(self, wallet, limit: int = 20, refresh: bool = True) -> list[OperationView]:
        address = wallet.address()
        return [
            self._view(item, address)
            for item in self.get_operations(wallet, limit=limit, refresh=refresh)
        ]

    def get_activity_views(
        self,
        wallet,
        limit: int = 20,
        refresh: bool = True,
    ) -> list[ActivityView]:
        """Group cached account operations into transaction-level activities."""
        address = wallet.address()
        if refresh:
            self.sync_recent(wallet)
        operation_limit = max(limit * 4, SYNC_PAGE_LIMIT if limit >= SYNC_PAGE_LIMIT else limit)
        raw_operations = self.datastore.get_operations(
            self.network_name,
            address,
            limit=operation_limit,
        )
        operations = [self._view(item, address) for item in raw_operations]
        return _group_activities(operations)[:limit]

    @staticmethod
    def _view(raw: dict, account: str) -> OperationView:
        operation_type = raw.get("type", "unknown")
        return OperationView(
            operation_type=operation_type,
            created_at=raw.get("created_at"),
            summary=_summary(raw, account),
            raw=raw,
        )


def _group_activities(operations: list[OperationView]) -> list[ActivityView]:
    buckets: dict[str, list[OperationView]] = {}
    order: list[str] = []
    for operation in operations:
        transaction_hash = operation.raw.get("transaction_hash")
        if transaction_hash:
            key = f"tx:{transaction_hash}"
        else:
            token = operation.raw.get("paging_token") or operation.raw.get("id")
            key = f"op:{token if token is not None else len(order)}"
        if key not in buckets:
            buckets[key] = []
            order.append(key)
        buckets[key].append(operation)

    activities = []
    for key in order:
        grouped = buckets[key]
        first = grouped[0]
        transaction_hash = first.raw.get("transaction_hash")
        count = len(grouped)
        activities.append(
            ActivityView(
                operation_type=first.operation_type if count == 1 else "transaction",
                created_at=first.created_at,
                summary=_activity_summary(grouped),
                transaction_hash=str(transaction_hash) if transaction_hash else None,
                operation_count=count,
                operations=grouped,
                raw=[item.raw for item in grouped],
            )
        )
    return activities


def _activity_summary(operations: list[OperationView]) -> str:
    if len(operations) == 1:
        return operations[0].summary
    summaries = [item.summary for item in operations]
    if len(summaries) == 2:
        return " · ".join(summaries)
    return f"{len(summaries)} actions · {summaries[0]} · {summaries[1]} · +{len(summaries) - 2} more"


def _summary(raw: dict, account: str) -> str:
    operation_type = raw.get("type")

    if operation_type == "payment":
        asset = _asset_from_fields(raw)
        amount = format_amount(raw.get("amount", "?"))
        source = raw.get("from") or raw.get("source_account") or "?"
        destination = raw.get("to") or "?"
        if destination == account:
            return f"Received {amount} {asset} from {short_address(source)}"
        if source == account:
            return f"Sent {amount} {asset} to {short_address(destination)}"
        return f"{amount} {asset}: {short_address(source)} -> {short_address(destination)}"

    if operation_type == "create_account":
        created = raw.get("account", "?")
        amount = format_amount(raw.get("starting_balance", "?"))
        funder = raw.get("funder") or raw.get("source_account") or "?"
        if created == account:
            return f"Account created with {amount} XLM"
        if funder == account:
            return f"Created {short_address(created)} with {amount} XLM"
        return f"Created {short_address(created)} with {amount} XLM"

    if operation_type in {"manage_sell_offer", "create_passive_sell_offer"}:
        offer_id = raw.get("offer_id") or raw.get("id") or "?"
        amount = format_amount(raw.get("amount", "?"))
        if amount == "0":
            return f"Cancelled offer #{offer_id}"
        selling = _asset_from_fields(raw, "selling_")
        buying = _asset_from_fields(raw, "buying_")
        price = format_amount(raw.get("price", "?"))
        verb = "Placed passive offer" if operation_type == "create_passive_sell_offer" else "Sell offer"
        return f"{verb}: {amount} {selling} -> {buying} @ {price}"

    if operation_type == "manage_buy_offer":
        offer_id = raw.get("offer_id") or raw.get("id") or "?"
        amount = format_amount(raw.get("buy_amount", "?"))
        if amount == "0":
            return f"Cancelled offer #{offer_id}"
        selling = _asset_from_fields(raw, "selling_")
        buying = _asset_from_fields(raw, "buying_")
        price = format_amount(raw.get("price", "?"))
        return f"Buy offer: {amount} {buying} with {selling} @ {price}"

    if operation_type == "change_trust":
        asset = _asset_from_fields(raw)
        if raw.get("asset_type") == "liquidity_pool_shares":
            pool_id = raw.get("liquidity_pool_id", "")
            asset = f"liquidity pool {pool_id[:8]}..." if pool_id else "liquidity pool"
        limit = format_amount(raw.get("limit", "?"))
        if limit == "0":
            return f"Removed trustline for {asset}"
        return f"Set trustline for {asset}"

    if operation_type == "liquidity_pool_deposit":
        reserves = _reserve_summary(raw.get("reserves_deposited", []))
        return f"Added liquidity: {reserves}" if reserves else "Added liquidity"

    if operation_type == "liquidity_pool_withdraw":
        reserves = _reserve_summary(raw.get("reserves_received", []))
        return f"Removed liquidity: {reserves}" if reserves else "Removed liquidity"

    if operation_type == "account_merge":
        destination = raw.get("into") or raw.get("account") or "?"
        return f"Merged account into {short_address(destination)}"

    if operation_type == "manage_data":
        name = raw.get("name", "data entry")
        return f"Updated account data: {name}"

    if operation_type == "set_options":
        return "Updated account settings"

    if operation_type == "bump_sequence":
        return f"Bumped sequence to {raw.get('bump_to', '?')}"

    return (operation_type or "unknown").replace("_", " ").capitalize()


def _asset_from_fields(raw: dict, prefix: str = "") -> str:
    asset_type = raw.get(f"{prefix}asset_type")
    if asset_type == "native":
        return "XLM"
    code = raw.get(f"{prefix}asset_code")
    issuer = raw.get(f"{prefix}asset_issuer")
    if code and issuer:
        return f"{code} ({short_address(issuer)})"
    return code or "asset"


def _reserve_summary(reserves) -> str:
    parts = []
    for reserve in reserves or []:
        asset = str(reserve.get("asset", "?"))
        if asset.lower() == "native":
            asset = "XLM"
        elif ":" in asset:
            code, issuer = asset.split(":", 1)
            asset = f"{code} ({short_address(issuer)})"
        parts.append(f"{format_amount(reserve.get('amount', '?'))} {asset}")
    return " + ".join(parts)


def _paging_token(raw: dict) -> str:
    token = raw.get("paging_token") or raw.get("id")
    return str(token) if token is not None else ""
