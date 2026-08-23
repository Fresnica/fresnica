"""Presentation-only helpers for Stellar DEX market data."""

from decimal import Decimal, InvalidOperation, localcontext

from .models import PriceRatio


SYNTHETIC_OFFER_TYPE_SHIFT = 62
SYNTHETIC_OFFER_TYPE_TOID = 1
SYNTHETIC_OFFER_ID_MASK = (1 << SYNTHETIC_OFFER_TYPE_SHIFT) - 1


def decode_synthetic_offer_id(value) -> int | None:
    """Return the Horizon operation TOID for an immediately filled offer."""
    try:
        encoded = int(str(value))
    except (TypeError, ValueError):
        return None
    if encoded < 0 or encoded >> SYNTHETIC_OFFER_TYPE_SHIFT != SYNTHETIC_OFFER_TYPE_TOID:
        return None
    return encoded & SYNTHETIC_OFFER_ID_MASK


def is_synthetic_offer_id(value) -> bool:
    return decode_synthetic_offer_id(value) is not None


def offer_id_label(value) -> str:
    if value is None:
        return "-"
    return "Immediate" if is_synthetic_offer_id(value) else str(value)


def format_market_price(value, max_decimals: int = 10) -> str:
    """Compact rounded display for a price while preserving exact data elsewhere."""
    try:
        number = value if isinstance(value, Decimal) else Decimal(str(value))
    except (InvalidOperation, TypeError, ValueError):
        return str(value)
    if not number.is_finite():
        return str(value)
    if number == 0:
        return "0"
    magnitude = abs(number)
    if Decimal("0.0000001") <= magnitude < Decimal("1000000000"):
        quantum = Decimal(1).scaleb(-max_decimals)
        with localcontext() as context:
            context.prec = max(28, len(number.as_tuple().digits) + max_decimals + 2)
            rounded = number.quantize(quantum)
        return format(rounded, "f").rstrip("0").rstrip(".")
    with localcontext() as context:
        context.prec = 12
        text = format(+number, ".10g")
    return text.replace("E", "e")


def format_price_ratio(price: PriceRatio, max_decimals: int = 10) -> str:
    with localcontext() as context:
        context.prec = 40
        value = Decimal(price.n) / Decimal(price.d)
    return format_market_price(value, max_decimals=max_decimals)
