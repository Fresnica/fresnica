"""User-facing Stellar payment orchestration."""

from .availability import AvailabilityService
from .errors import WatchOnlyError
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
        contact_name: str | None = None,
    ):
        if not wallet.can_sign():
            raise WatchOnlyError("Watch-only wallet cannot transfer")

        asset = asset if isinstance(asset, Asset) else Asset.parse(asset)

        account = self.balance_service.get_account(wallet, refresh=True)
        adapter = self.balance_service.adapter
        base_fee = adapter.fetch_base_fee()
        base_reserve = adapter.get_base_reserve_stroops()
        amount = self.availability.validate_transfer(
            account,
            asset,
            amount,
            base_reserve_stroops=base_reserve,
            fee_stroops=base_fee,
        )

        return self.transaction_builder.build_payment(
            wallet_name=wallet_name,
            wallet=wallet,
            destination=destination,
            asset=asset,
            amount=amount,
            base_fee_stroops=base_fee,
            memo=memo,
            contact_name=contact_name,
        )

    def sign(self, wallet, prepared):
        return self.transaction_service.sign(wallet, prepared)

    def submit(self, prepared):
        return self.transaction_service.submit(prepared)
