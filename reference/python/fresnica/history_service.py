"""Account activity with local caching and human-readable summaries."""

from decimal import Decimal, InvalidOperation

from .models import ActivityView, OperationView
from .presentation import format_amount, short_address
from .sync import SyncResult


SYNC_PAGE_LIMIT = 200
SYNC_MAX_INCREMENTAL_PAGES = 5
SUSPICIOUS_NATIVE_DUST_MAX = Decimal("0.0000010")


class HistoryService:
    def __init__(self, adapter, datastore, network_name: str):
        self.adapter = adapter
        self.datastore = datastore
        self.network_name = network_name

    def sync_recent(self, wallet, limit: int = SYNC_PAGE_LIMIT) -> SyncResult:
        """Catch up newer operations with a bounded forward pagination loop."""
        address = wallet.address()
        cached = self.datastore.get_operations(self.network_name, address, limit=1)
        if not cached:
            response = self.adapter.get_operations(address, limit=limit, desc=True)
            records = list(response.get("_embedded", {}).get("records", []))
            if records:
                self.datastore.save_operations(self.network_name, address, response)
            # A descending first page is anchored at Horizon head even when older
            # history exists; older data is explicitly fetched via load_older().
            return SyncResult(fetched_count=len(records), caught_up=True)

        next_cursor = _paging_token(cached[0])
        fetched = 0
        for _ in range(SYNC_MAX_INCREMENTAL_PAGES):
            response = self.adapter.get_operations(
                address,
                limit=limit,
                cursor=next_cursor,
                desc=False,
            )
            records = list(response.get("_embedded", {}).get("records", []))
            if not records:
                return SyncResult(fetched_count=fetched, caught_up=True)
            self.datastore.save_operations(self.network_name, address, response)
            fetched += len(records)
            last_cursor = _paging_token(records[-1])
            if not last_cursor or last_cursor == next_cursor:
                return SyncResult(fetched_count=fetched, caught_up=False)
            next_cursor = last_cursor
            if len(records) < limit:
                return SyncResult(fetched_count=fetched, caught_up=True)
        return SyncResult(fetched_count=fetched, caught_up=False)

    def load_older(self, wallet, limit: int = SYNC_PAGE_LIMIT) -> int:
        address = wallet.address()
        cached = self.datastore.get_operations(self.network_name, address, limit=100000)
        if not cached:
            return self.sync_recent(wallet, limit=limit).fetched_count
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

    def cached_operation_count(self, wallet) -> int:
        return len(
            self.datastore.get_operations(
                self.network_name,
                wallet.address(),
                limit=100000,
            )
        )

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
        # Multi-operation transactions mean N displayed activities can require more
        # than N raw operations. Read the full local cache so loading older pages is
        # immediately reflected instead of being hidden behind a fixed 200-op window.
        raw_operations = self.datastore.get_operations(
            self.network_name,
            address,
            limit=100000,
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


def is_suspicious_claimable_activity(activity: ActivityView) -> bool:
    """Recognize unsolicited claimables and their same-sender stroop bait.

    A claimable-only transaction is suspicious. A mixed transaction is also
    suspicious when every sibling operation is a tiny incoming native payment
    from the same sender to the same claimant. Other mixed transactions remain
    visible, so legitimate sibling operations such as clawbacks are not hidden.
    """
    operations = list(activity.operations)
    claimables = [item for item in operations if _is_unsolicited_claimable(item)]
    if not claimables:
        return False
    if len(claimables) == len(operations):
        return True

    sources = {
        str(item.raw.get("source_account"))
        for item in claimables
        if item.raw.get("source_account")
    }
    claimants = {
        str(claimant.get("destination"))
        for item in claimables
        for claimant in item.raw.get("claimants", []) or []
        if isinstance(claimant, dict) and claimant.get("destination")
    }
    if not sources or not claimants:
        return False
    return all(
        _is_unsolicited_claimable(item)
        or _is_suspicious_native_companion(item, sources, claimants)
        for item in operations
    )


def _is_unsolicited_claimable(operation: OperationView) -> bool:
    return (
        operation.operation_type == "create_claimable_balance"
        and bool(operation.raw.get("_fresnica_unsolicited_claimable"))
    )


def _is_suspicious_native_companion(
    operation: OperationView,
    sources: set[str],
    claimants: set[str],
) -> bool:
    if operation.operation_type != "payment":
        return False
    raw = operation.raw
    source = raw.get("from") or raw.get("source_account")
    if str(source or "") not in sources or str(raw.get("to") or "") not in claimants:
        return False
    if raw.get("asset_type") != "native":
        return False
    try:
        amount = Decimal(str(raw.get("amount", "")))
    except InvalidOperation:
        return False
    return Decimal("0") < amount <= SUSPICIOUS_NATIVE_DUST_MAX


# Compatibility for callers outside the TUI while the old name ages out.
def is_dust_activity(activity: ActivityView) -> bool:
    return is_suspicious_claimable_activity(activity)


def activity_counterparties(activity: ActivityView, account: str) -> list[str]:
    """Return G-addresses useful for contact creation from an activity."""
    values: list[str] = []
    for operation in activity.operations:
        raw = operation.raw
        candidates = [
            raw.get("from"),
            raw.get("to"),
            raw.get("source_account"),
            raw.get("funder"),
            raw.get("account"),
            raw.get("into"),
        ]
        candidates.extend(
            claimant.get("destination")
            for claimant in raw.get("claimants", []) or []
            if isinstance(claimant, dict)
        )
        for change in raw.get("asset_balance_changes", []) or []:
            if isinstance(change, dict):
                candidates.extend([change.get("from"), change.get("to")])
        for value in candidates:
            if (
                isinstance(value, str)
                and value.startswith("G")
                and value != account
                and value not in values
            ):
                values.append(value)
    return values


def activity_summary_for_display(
    activity: ActivityView,
    account: str,
    contact_names: dict[str, str] | None = None,
    issuer_domains: dict[str, str] | None = None,
) -> str:
    """Rebuild an activity summary with current local presentation metadata.

    Raw operations remain canonical. Contact names and issuer domains are
    presentation-time metadata, so adding a contact can update history
    immediately without rewriting cached Horizon records.
    """
    operations = [
        OperationView(
            operation_type=item.operation_type,
            created_at=item.created_at,
            summary=_summary(
                item.raw,
                account,
                contact_names=contact_names,
                issuer_domains=issuer_domains,
            ),
            raw=item.raw,
        )
        for item in activity.operations
    ]
    return _activity_summary(operations) if operations else activity.summary


def _activity_summary(operations: list[OperationView]) -> str:
    if len(operations) == 1:
        return operations[0].summary
    summaries = [item.summary for item in operations]
    if len(summaries) == 2:
        return " · ".join(summaries)
    return f"{len(summaries)} actions · {summaries[0]} · {summaries[1]} · +{len(summaries) - 2} more"


def _summary(
    raw: dict,
    account: str,
    contact_names: dict[str, str] | None = None,
    issuer_domains: dict[str, str] | None = None,
) -> str:
    operation_type = raw.get("type")

    if operation_type == "payment":
        asset = _asset_from_fields(raw, issuer_domains=issuer_domains)
        amount = format_amount(raw.get("amount", "?"))
        source = raw.get("from") or raw.get("source_account") or "?"
        destination = raw.get("to") or "?"
        if destination == account:
            return f"Received {amount} {asset} from {_address(source, contact_names)}"
        if source == account:
            return f"Sent {amount} {asset} to {_address(destination, contact_names)}"
        return f"{amount} {asset}: {_address(source, contact_names)} -> {_address(destination, contact_names)}"

    if operation_type == "create_account":
        created = raw.get("account", "?")
        amount = format_amount(raw.get("starting_balance", "?"))
        funder = raw.get("funder") or raw.get("source_account") or "?"
        if created == account:
            return f"Account created with {amount} XLM"
        if funder == account:
            return f"Created {_address(created, contact_names)} with {amount} XLM"
        return f"Created {_address(created, contact_names)} with {amount} XLM"

    if operation_type in {"manage_sell_offer", "create_passive_sell_offer"}:
        offer_id = str(raw.get("offer_id") or "0")
        amount = format_amount(raw.get("amount", "?"))
        if amount == "0":
            return f"Cancelled offer #{offer_id}"
        selling = _asset_from_fields(raw, "selling_", issuer_domains=issuer_domains)
        buying = _asset_from_fields(raw, "buying_", issuer_domains=issuer_domains)
        price = format_amount(raw.get("price", "?"))
        if operation_type == "create_passive_sell_offer":
            return f"Placed passive SELL {amount} {selling} @ {price} {buying}/{selling}"
        verb = "Placed" if offer_id == "0" else f"Updated #{offer_id}"
        return f"{verb} SELL {amount} {selling} @ {price} {buying}/{selling}"

    if operation_type == "manage_buy_offer":
        offer_id = str(raw.get("offer_id") or "0")
        # Horizon's Manage Buy Offer operation object calls this field `amount`:
        # it is the quantity of buying_asset the account wants to buy.
        amount = format_amount(raw.get("amount", "?"))
        if amount == "0":
            return f"Cancelled offer #{offer_id}"
        selling = _asset_from_fields(raw, "selling_", issuer_domains=issuer_domains)
        buying = _asset_from_fields(raw, "buying_", issuer_domains=issuer_domains)
        price = format_amount(raw.get("price", "?"))
        verb = "Placed" if offer_id == "0" else f"Updated #{offer_id}"
        return f"{verb} BUY {amount} {buying} @ {price} {selling}/{buying}"

    if operation_type == "change_trust":
        asset = _asset_from_fields(raw, issuer_domains=issuer_domains)
        if raw.get("asset_type") == "liquidity_pool_shares":
            pool_id = raw.get("liquidity_pool_id", "")
            asset = f"liquidity pool {pool_id[:8]}..." if pool_id else "liquidity pool"
        limit = format_amount(raw.get("limit", "?"))
        if limit == "0":
            return f"Removed trustline for {asset}"
        return f"Set trustline for {asset} · limit {limit}"

    if operation_type == "create_claimable_balance":
        source = raw.get("source_account") or "?"
        amount = format_amount(raw.get("amount", "?"))
        asset = _asset_from_sep11(raw.get("asset"), issuer_domains)
        claimants = [
            item.get("destination")
            for item in raw.get("claimants", []) or []
            if isinstance(item, dict)
        ]
        if source == account:
            recipients = [item for item in claimants if item and item != account]
            target = _address(recipients[0], contact_names) if recipients else "claimant"
            return f"Created claimable payment: {amount} {asset} for {target}"
        if account in claimants:
            raw["_fresnica_unsolicited_claimable"] = True
            return (
                f"Incoming claimable asset: {amount} {asset} from {_address(source, contact_names)} "
                "· review before claiming"
            )
        return f"Claimable asset created: {amount} {asset}"

    if operation_type == "claim_claimable_balance":
        balance_id = str(raw.get("balance_id") or raw.get("id") or "?")
        return f"Claimed claimable balance {balance_id[:12]}..."

    if operation_type == "clawback":
        amount = format_amount(raw.get("amount", "?"))
        asset = _asset_from_fields(raw, issuer_domains=issuer_domains)
        source = raw.get("source_account") or "?"
        target = raw.get("from") or "?"
        if target == account:
            return f"Clawback: {amount} {asset} reclaimed from your account"
        if source == account:
            return f"Clawed back {amount} {asset} from {_address(target, contact_names)}"
        return f"Clawback {amount} {asset} from {_address(target, contact_names)}"

    if operation_type == "clawback_claimable_balance":
        balance_id = str(raw.get("balance_id") or raw.get("id") or "?")
        return f"Clawed back claimable balance {balance_id[:12]}..."

    if operation_type == "invoke_host_function":
        return _contract_summary(raw, account, contact_names, issuer_domains)

    if operation_type == "liquidity_pool_deposit":
        reserves = _reserve_summary(raw.get("reserves_deposited", []), issuer_domains)
        return f"Added liquidity: {reserves}" if reserves else "Added liquidity"

    if operation_type == "liquidity_pool_withdraw":
        reserves = _reserve_summary(raw.get("reserves_received", []), issuer_domains)
        return f"Removed liquidity: {reserves}" if reserves else "Removed liquidity"

    if operation_type == "account_merge":
        destination = raw.get("into") or raw.get("account") or "?"
        return f"Merged account into {_address(destination, contact_names)}"

    if operation_type == "manage_data":
        name = raw.get("name", "data entry")
        return f"Updated account data: {name}"

    if operation_type == "set_options":
        return "Updated account settings"

    if operation_type == "bump_sequence":
        return f"Bumped sequence to {raw.get('bump_to', '?')}"

    return (operation_type or "unknown").replace("_", " ").capitalize()


def _contract_summary(raw, account, contact_names, issuer_domains) -> str:
    function = str(raw.get("function") or "")
    if "CreateContract" in function:
        return "Contract deployment"
    if "UploadContractWasm" in function:
        return "Contract code upload"

    changes = [
        item
        for item in raw.get("asset_balance_changes", []) or []
        if isinstance(item, dict)
    ]
    if not changes:
        return "Contract call"

    summaries = [
        _contract_asset_change(item, account, contact_names, issuer_domains)
        for item in changes
    ]
    if len(summaries) == 1:
        return f"Contract call · {summaries[0]}"
    return f"Contract call · {len(summaries)} asset changes · {summaries[0]}"


def _contract_asset_change(change, account, contact_names, issuer_domains) -> str:
    amount = format_amount(change.get("amount", "?"))
    asset = _asset_from_balance_change(change, issuer_domains)
    kind = str(change.get("type") or "change").lower()
    source = change.get("from")
    destination = change.get("to")

    if kind == "transfer":
        if destination == account:
            return f"Received {amount} {asset} from {_address(source, contact_names)}"
        if source == account:
            return f"Sent {amount} {asset} to {_address(destination, contact_names)}"
        return f"Transfer {amount} {asset}"
    if kind == "mint":
        if destination == account:
            return f"Minted {amount} {asset} to your account"
        return f"Minted {amount} {asset} to {_address(destination, contact_names)}"
    if kind == "burn":
        if source == account:
            return f"Burned {amount} {asset} from your account"
        return f"Burned {amount} {asset} from {_address(source, contact_names)}"
    if kind == "clawback":
        if source == account:
            return f"Clawback: {amount} {asset} reclaimed from your account"
        return f"Clawback {amount} {asset} from {_address(source, contact_names)}"
    return f"{kind.capitalize()} {amount} {asset}"


def _address(value, contact_names: dict[str, str] | None) -> str:
    text = str(value or "?")
    if contact_names and text in contact_names:
        return f"👤 {contact_names[text]} · {short_address(text)}"
    return short_address(text)


def _asset_from_balance_change(raw: dict, issuer_domains: dict[str, str] | None) -> str:
    if raw.get("asset_type") == "native":
        return "XLM"
    code = raw.get("asset_code") or raw.get("code")
    issuer = raw.get("asset_issuer") or raw.get("issuer")
    return _issued_asset(code, issuer, issuer_domains)


def _asset_from_sep11(value, issuer_domains: dict[str, str] | None = None) -> str:
    text = str(value or "asset")
    if text.lower() == "native":
        return "XLM"
    if ":" in text:
        code, issuer = text.split(":", 1)
        return _issued_asset(code, issuer, issuer_domains)
    return text


def _asset_from_fields(
    raw: dict,
    prefix: str = "",
    issuer_domains: dict[str, str] | None = None,
) -> str:
    asset_type = raw.get(f"{prefix}asset_type")
    if asset_type == "native":
        return "XLM"
    code = raw.get(f"{prefix}asset_code")
    issuer = raw.get(f"{prefix}asset_issuer")
    if code and issuer:
        return _issued_asset(code, issuer, issuer_domains)
    return code or "asset"


def _issued_asset(code, issuer, issuer_domains: dict[str, str] | None) -> str:
    if not code:
        return "asset"
    if not issuer:
        return str(code)
    issuer = str(issuer)
    domain = str((issuer_domains or {}).get(issuer) or "").strip()
    if domain:
        return f"{code} · {domain}"
    return f"{code} ({short_address(issuer)})"


def _reserve_summary(reserves, issuer_domains=None) -> str:
    parts = []
    for reserve in reserves or []:
        asset = _asset_from_sep11(reserve.get("asset", "?"), issuer_domains)
        parts.append(f"{format_amount(reserve.get('amount', '?'))} {asset}")
    return " + ".join(parts)


def _paging_token(raw: dict) -> str:
    token = raw.get("paging_token") or raw.get("id")
    return str(token) if token is not None else ""
