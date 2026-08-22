"""Human-facing formatting shared by CLI and TUI presentations."""

from datetime import datetime
from decimal import Decimal, InvalidOperation

from .models import Asset


def format_amount(value) -> str:
    if value is None:
        return "-"
    try:
        number = value if isinstance(value, Decimal) else Decimal(str(value))
    except (InvalidOperation, ValueError, TypeError):
        return str(value)
    if not number.is_finite():
        return str(value)
    if number == 0:
        return "0"
    text = format(number, "f")
    if "." in text:
        text = text.rstrip("0").rstrip(".")
    return text or "0"


def short_address(value: str | None, head: int = 6, tail: int = 4) -> str:
    if not value:
        return "-"
    if len(value) <= head + tail + 3:
        return value
    return f"{value[:head]}...{value[-tail:]}"


def short_pool_id(value: str | None) -> str:
    if not value:
        return "-"
    if len(value) <= 16:
        return value
    return f"{value[:8]}...{value[-6:]}"


def asset_source(asset: Asset) -> str:
    if asset.is_native:
        return "Native"
    if asset.is_liquidity_pool:
        return f"Pool {short_pool_id(asset.liquidity_pool_id)}"
    return short_address(asset.issuer)


def asset_label(asset: Asset, include_source: bool = False) -> str:
    if asset.is_native:
        return "XLM"
    if asset.is_liquidity_pool:
        return f"LP {short_pool_id(asset.liquidity_pool_id)}"
    if include_source:
        return f"{asset.code} ({short_address(asset.issuer)})"
    return asset.code


def format_timestamp(value: str | None, compact: bool = True) -> str:
    if not value:
        return ""
    try:
        parsed = datetime.fromisoformat(value.replace("Z", "+00:00"))
    except ValueError:
        return value
    if compact:
        return parsed.strftime("%m-%d %H:%M")
    return parsed.strftime("%Y-%m-%d %H:%M:%S")
