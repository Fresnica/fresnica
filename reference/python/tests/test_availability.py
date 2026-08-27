from decimal import Decimal

import pytest

from fresnica.availability import AvailabilityService
from fresnica.errors import InsufficientBalanceError, InvalidAmountError, TransactionError
from fresnica.models import Asset


ACCOUNT = {
    "account_id": "GWALLET",
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
            "limit": "200",
            "is_authorized": True,
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


def test_receiving_capacity_uses_limit_and_buying_liabilities():
    service = AvailabilityService()
    assert service.receiving_capacity(ACCOUNT, Asset("USDC", "GISSUER")) == Decimal("95")


def test_native_receiving_capacity_uses_int64_headroom():
    service = AvailabilityService()
    account = {
        "account_id": "GWALLET",
        "balances": [
            {
                "asset_type": "native",
                "balance": "922337203685.4775800",
                "selling_liabilities": "0",
                "buying_liabilities": "0",
            }
        ],
    }
    assert service.receiving_capacity(account, Asset("XLM")) == Decimal("0.0000007")
    service.validate_receive(account, Asset("XLM"), "0.0000007")
    with pytest.raises(InsufficientBalanceError):
        service.validate_receive(account, Asset("XLM"), "0.0000008")


def test_issuer_own_asset_uses_protocol_amount_limit_without_self_trustline():
    service = AvailabilityService()
    account = {
        "account_id": "GISSUER",
        "subentry_count": 0,
        "num_sponsoring": 0,
        "num_sponsored": 0,
        "balances": [
            {
                "asset_type": "native",
                "balance": "10",
                "selling_liabilities": "0",
                "buying_liabilities": "0",
            }
        ],
    }
    asset = Asset("USD", "GISSUER")
    assert service.available_for_transfer(account, asset, 5_000_000, 100) == Decimal(
        "922337203685.4775807"
    )
    assert service.receiving_capacity(account, asset) == Decimal("922337203685.4775807")


def test_payment_authorization_requires_full_authorization_not_maintain_only():
    service = AvailabilityService()
    account = {**ACCOUNT, "balances": [dict(item) for item in ACCOUNT["balances"]]}
    account["balances"][1]["is_authorized"] = False
    account["balances"][1]["is_authorized_to_maintain_liabilities"] = True
    with pytest.raises(TransactionError, match="not fully authorized"):
        service.validate_transfer(account, Asset("USDC", "GISSUER"), "1", 5_000_000, 100)
    with pytest.raises(TransactionError, match="not fully authorized"):
        service.validate_receive(account, Asset("USDC", "GISSUER"), "1")
