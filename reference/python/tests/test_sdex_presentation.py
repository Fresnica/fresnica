from decimal import Decimal

from fresnica.models import PriceRatio
from fresnica.sdex_presentation import (
    decode_synthetic_offer_id,
    format_market_price,
    format_price_ratio,
    format_stellar_decimal,
    offer_id_label,
    stellar_decimal_parts,
    stellar_price_ratio_parts,
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


def test_stellar_decimal_is_fixed_seven_places_with_dimmable_padding():
    assert stellar_decimal_parts(Decimal("0.325")) == ("0.325", "0000")
    assert format_stellar_decimal(Decimal("0.325")) == "0.3250000"
    assert stellar_decimal_parts(Decimal("12.34567894")) == ("12.3456789", "")
    assert stellar_decimal_parts(Decimal("1")) == ("1.", "0000000")


def test_nonzero_sub_stroop_price_never_looks_like_zero():
    assert stellar_decimal_parts(Decimal("0.00000001")) == ("<0.0000001", "")


def test_price_ratio_uses_same_fixed_seven_decimal_rule():
    assert stellar_price_ratio_parts(PriceRatio(1, 3)) == ("0.3333333", "")
    assert stellar_price_ratio_parts(PriceRatio(13, 40)) == ("0.325", "0000")
