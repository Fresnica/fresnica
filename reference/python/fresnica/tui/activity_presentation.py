"""Presentation helpers shared by dashboard and full account history."""

from rich.text import Text

from ..balance_service import ISSUER_DOMAIN_CACHE_KEY
from ..errors import FresnicaError
from ..history_service import activity_summary_for_display, is_suspicious_claimable_activity


def activity_metadata(runtime, wallet) -> tuple[dict[str, str], dict[str, str]]:
    """Return current local contact and issuer labels without network I/O."""
    contacts: dict[str, str] = {}
    store = getattr(runtime, "contact_store", None)
    if store is not None:
        try:
            contacts = {item.address: item.name for item in store.list()}
        except FresnicaError:
            contacts = {}

    domains: dict[str, str] = {}
    try:
        record = runtime.wallet_manager.get_record()
        service = runtime.services_for(record.network).balance_service
        getter = getattr(service, "get_cached_portfolio_views", None)
        if getter is not None:
            balances, _ = getter(wallet)
            for balance in balances:
                issuer = balance.asset.issuer
                if not issuer or not isinstance(balance.raw, dict):
                    continue
                domain = str(balance.raw.get(ISSUER_DOMAIN_CACHE_KEY) or "").strip()
                if domain:
                    domains[issuer] = domain
    except (FresnicaError, ValueError, AttributeError):
        pass
    return contacts, domains


def activity_display_summary(activity, account: str, contacts, domains) -> str:
    if hasattr(activity, "operations"):
        return activity_summary_for_display(
            activity,
            account,
            contact_names=contacts,
            issuer_domains=domains,
        )
    return activity.summary


def activity_text(activity, summary: str, account: str) -> Text:
    """Add low-noise semantic color while leaving the summary readable as text."""
    suspicious = bool(
        hasattr(activity, "operations") and is_suspicious_claimable_activity(activity)
    )
    text = Text()
    if suspicious:
        text.append("⚠ ", style="yellow")
        text.append(summary, style="dim")
        return text

    marker, style = _marker(activity, account)
    text.append(marker, style=style)
    text.append(summary)
    return text


def _marker(activity, account: str) -> tuple[str, str]:
    operations = list(getattr(activity, "operations", []) or [])
    if len(operations) != 1:
        return "• ", "dim"
    operation = operations[0]
    raw = operation.raw
    kind = operation.operation_type

    if kind == "payment":
        if raw.get("to") == account:
            return "↓ ", "green"
        if (raw.get("from") or raw.get("source_account")) == account:
            return "↑ ", "cyan"
    if kind == "invoke_host_function":
        for change in raw.get("asset_balance_changes", []) or []:
            if not isinstance(change, dict):
                continue
            if change.get("to") == account:
                return "↓ ", "green"
            if change.get("from") == account:
                return "↑ ", "cyan"
        return "◆ ", "magenta"
    if kind in {"manage_sell_offer", "manage_buy_offer", "create_passive_sell_offer"}:
        return "⇄ ", "blue"
    if kind in {"change_trust"}:
        return "◇ ", "yellow"
    if kind in {"clawback", "clawback_claimable_balance"}:
        return "↶ ", "yellow"
    if kind in {"liquidity_pool_deposit", "liquidity_pool_withdraw"}:
        return "◈ ", "blue"
    return "• ", "dim"
