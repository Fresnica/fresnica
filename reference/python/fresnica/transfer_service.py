"""User-facing Stellar payment orchestration."""

import base64
import binascii
from decimal import Decimal

from .availability import AvailabilityService, STROOPS_PER_XLM
from .errors import InvalidAmountError, MemoRequiredError, TransactionError, WatchOnlyError
from .models import Asset


class TransferService:
    def __init__(
        self,
        balance_service,
        transaction_builder,
        transaction_service,
    ):
        self.balance_service = balance_service
        self.transaction_builder = transaction_builder
        self.transaction_service = transaction_service
        self.availability = AvailabilityService()

    def prepare(
        self,
        wallet_name: str,
        wallet,
        destination: str,
        asset,
        amount,
        memo: str | None = None,
        memo_type: str | None = None,
        contact_name: str | None = None,
    ):
        if not wallet.can_sign():
            raise WatchOnlyError("Watch-only wallet cannot transfer")

        asset = asset if isinstance(asset, Asset) else Asset.parse(asset)

        account = self.balance_service.get_account(wallet, refresh=True)
        adapter = self.balance_service.adapter
        create_destination = not adapter.account_exists(destination)
        if create_destination and not asset.is_native:
            raise TransactionError(
                "Destination account does not exist. Only XLM can create a new "
                "Stellar account; issued assets require an existing account and trustline."
            )
        destination_account = None if create_destination else adapter.get_account(destination)

        base_fee = adapter.fetch_base_fee()
        base_reserve = adapter.get_base_reserve_stroops()
        amount = self.availability.validate_transfer(
            account,
            asset,
            amount,
            base_reserve_stroops=base_reserve,
            fee_stroops=base_fee,
        )
        if destination_account is not None:
            self.availability.validate_receive(destination_account, asset, amount)
            if not memo and _account_requires_memo(destination_account):
                raise MemoRequiredError(destination)

        if create_destination:
            minimum = Decimal(2 * base_reserve) / STROOPS_PER_XLM
            if amount < minimum:
                raise InvalidAmountError(
                    "Creating a Stellar account requires at least "
                    f"{minimum.normalize()} XLM at the current base reserve; "
                    f"requested {amount} XLM"
                )

        return self.transaction_builder.build_payment(
            wallet_name=wallet_name,
            wallet=wallet,
            destination=destination,
            asset=asset,
            amount=amount,
            base_fee_stroops=base_fee,
            memo=memo,
            memo_type=memo_type,
            contact_name=contact_name,
            create_destination=create_destination,
        )

    def sign(self, wallet, prepared):
        return self.transaction_service.sign(wallet, prepared)

    def submit(self, prepared):
        return self.transaction_service.submit(prepared)


def _account_requires_memo(account: dict) -> bool:
    encoded = (account.get("data") or {}).get("config.memo_required")
    if encoded is None:
        return False
    if not isinstance(encoded, str):
        raise TransactionError("Horizon returned malformed config.memo_required data")
    try:
        decoded = base64.b64decode(encoded, validate=True)
    except (binascii.Error, ValueError) as exc:
        raise TransactionError(
            "Horizon returned malformed config.memo_required data"
        ) from exc
    return decoded == b"1"
