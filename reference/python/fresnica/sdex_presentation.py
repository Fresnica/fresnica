"""Presentation-only helpers for Stellar DEX market data."""

from decimal import Decimal, InvalidOperation, ROUND_HALF_UP, localcontext

from .models import PriceRatio


SYNTHETIC_OFFER_TYPE_SHIFT = 62
SYNTHETIC_OFFER_TYPE_TOID = 1
SYNTHETIC_OFFER_ID_MASK = (1 << SYNTHETIC_OFFER_TYPE_SHIFT) - 1
STELLAR_DECIMAL_PLACES = 7
STELLAR_DECIMAL_QUANTUM = Decimal("0.0000001")


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


def stellar_decimal_parts(value) -> tuple[str, str]:
    """Return Fex-style fixed-7-decimal significant text and dimmable zero padding.

    A positive non-zero market price must never be rendered as 0.0000000 merely
    because it is below display precision.
    """
    try:
        number = value if isinstance(value, Decimal) else Decimal(str(value))
    except (InvalidOperation, TypeError, ValueError):
        return str(value), ""
    if not number.is_finite():
        return str(value), ""
    if number > 0 and number < STELLAR_DECIMAL_QUANTUM / 2:
        return "<0.0000001", ""

    with localcontext() as context:
        context.prec = max(28, len(number.as_tuple().digits) + 10)
        rounded = number.quantize(STELLAR_DECIMAL_QUANTUM, rounding=ROUND_HALF_UP)
    formatted = f"{rounded:,.7f}"
    zero_count = len(formatted) - len(formatted.rstrip("0"))
    if zero_count == 0:
        return formatted, ""
    return formatted[:-zero_count], formatted[-zero_count:]


def format_stellar_decimal(value) -> str:
    significant, padding = stellar_decimal_parts(value)
    return significant + padding


def format_market_price(value, max_decimals: int = 10) -> str:
    """Compact legacy display used outside fixed-width market tables."""
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


def stellar_price_ratio_parts(price: PriceRatio) -> tuple[str, str]:
    with localcontext() as context:
        context.prec = 40
        value = Decimal(price.n) / Decimal(price.d)
    return stellar_decimal_parts(value)
