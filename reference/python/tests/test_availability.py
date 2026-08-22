from decimal import Decimal

import pytest

from fresnica.availability import AvailabilityService
from fresnica.errors import InsufficientBalanceError, InvalidAmountError
from fresnica.models import Asset


ACCOUNT = {
    "subentry_count": 2,
    "num_sponsoring": 1,
    "num_sponsored": 1,
    "balances": [
        {
            "asset_type": "native",
            "balance": "10",
            "selling_liabilities": "1",
            "buying_liabilities": "0",
        },
        {
            "asset_type": "credit_alphanum4",
            "asset_code": "USDC",
            "asset_issuer": "GISSUER",
            "balance": "100",
            "selling_liabilities": "20",
            "buying_liabilities": "5",
        },
    ],
}


def test_xlm_available_includes_reserve_and_selling_liabilities():
    service = AvailabilityService()
    # 2 + 2 subentries + 1 sponsoring - 1 sponsored = 4 reserves = 2 XLM.
    value = service.available_for_transfer(
        ACCOUNT, Asset("XLM"), base_reserve_stroops=5_000_000, fee_stroops=100
    )
    assert value == Decimal("6.99999")


def test_credit_available_uses_selling_liabilities():
    service = AvailabilityService()
    value = service.available_for_transfer(
        ACCOUNT,
        Asset("USDC", "GISSUER"),
        base_reserve_stroops=5_000_000,
        fee_stroops=100,
    )
    assert value == Decimal("80")


def test_transfer_rejects_amount_over_available():
    service = AvailabilityService()
    with pytest.raises(InsufficientBalanceError):
        service.validate_transfer(
            ACCOUNT,
            Asset("USDC", "GISSUER"),
            "80.0000001",
            base_reserve_stroops=5_000_000,
            fee_stroops=100,
        )


def test_amount_rejects_more_than_seven_decimal_places():
    service = AvailabilityService()
    with pytest.raises(InvalidAmountError):
        service.validate_transfer(
            ACCOUNT,
            Asset("XLM"),
            "1.00000001",
            base_reserve_stroops=5_000_000,
            fee_stroops=100,
        )
