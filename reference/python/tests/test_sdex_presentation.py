from decimal import Decimal

from fresnica.models import PriceRatio
from fresnica.sdex_presentation import (
    decode_synthetic_offer_id,
    format_market_price,
    format_price_ratio,
    offer_id_label,
)


def test_horizon_synthetic_offer_id_decodes_immediate_operation_toid():
    value = "4885936293211082753"
    assert decode_synthetic_offer_id(value) == 274250274783694849
    assert offer_id_label(value) == "Immediate"
    assert decode_synthetic_offer_id("42") is None
    assert offer_id_label("42") == "42"


def test_market_price_display_bounds_recurring_decimal_without_changing_ratio():
    ratio = PriceRatio(1, 3)
    assert format_price_ratio(ratio) == "0.3333333333"
    assert format_market_price(Decimal("0.325000000000000000000")) == "0.325"
    assert format_market_price(Decimal("0.000000012345678901")) == "1.234567890e-8"
