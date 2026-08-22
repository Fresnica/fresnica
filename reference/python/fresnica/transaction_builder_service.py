"""Turn wallet-level payment intent into an SDK transaction envelope."""

from dataclasses import dataclass
from decimal import Decimal

from .availability import STROOPS_PER_XLM
from .models import Asset
from .review import TransactionReview


@dataclass
class PreparedTransaction:
    envelope: object
    review: TransactionReview


class TransactionBuilderService:
    def __init__(self, adapter):
        self.adapter = adapter

    def build_payment(
        self,
        wallet_name: str,
        wallet,
        destination: str,
        asset: Asset,
        amount: Decimal,
        base_fee_stroops: int,
        memo: str | None = None,
        contact_name: str | None = None,
    ) -> PreparedTransaction:
        envelope = self.adapter.build_payment(
            source=wallet.address(),
            destination=destination,
            asset=asset,
            amount=_amount_text(amount),
            base_fee=base_fee_stroops,
            memo=memo,
        )
        fee_xlm = Decimal(base_fee_stroops) / STROOPS_PER_XLM
        review = TransactionReview(
            wallet_name=wallet_name,
            source=wallet.address(),
            destination=destination,
            asset=asset.display,
            amount=_amount_text(amount),
            fee=_amount_text(fee_xlm),
            network=self.adapter.network.name,
            memo=memo,
            contact_name=contact_name,
        )
        return PreparedTransaction(envelope=envelope, review=review)


def _amount_text(value: Decimal) -> str:
    text = format(value, "f")
    if "." in text:
        text = text.rstrip("0").rstrip(".")
    return text or "0"
