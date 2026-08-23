"""Turn wallet-level transaction intent into an SDK transaction envelope."""

from dataclasses import dataclass
from decimal import Decimal

from .availability import STROOPS_PER_XLM
from .models import Asset, OfferIntent, OpenOffer
from .review import OfferReview, TransactionReview


@dataclass
class PreparedTransaction:
    envelope: object
    review: TransactionReview | OfferReview


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
        create_destination: bool = False,
    ) -> PreparedTransaction:
        envelope = self.adapter.build_payment(
            source=wallet.address(),
            destination=destination,
            asset=asset,
            amount=_amount_text(amount),
            base_fee=base_fee_stroops,
            memo=memo,
            create_destination=create_destination,
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
            operation="create_account" if create_destination else "payment",
            memo=memo,
            contact_name=contact_name,
        )
        return PreparedTransaction(envelope=envelope, review=review)

    def build_offer(
        self,
        wallet_name: str,
        wallet,
        intent: OfferIntent,
        base_fee_stroops: int,
        offer_id: int = 0,
        action: str = "create",
    ) -> PreparedTransaction:
        amount = _amount_text(intent.amount)
        price = _amount_text(intent.price)
        if intent.side == "buy":
            selling = intent.pair.counter
            buying = intent.pair.base
            envelope = self.adapter.build_manage_buy_offer(
                source=wallet.address(),
                selling=selling,
                buying=buying,
                buy_amount=amount,
                price=price,
                base_fee=base_fee_stroops,
                offer_id=offer_id,
            )
        else:
            selling = intent.pair.base
            buying = intent.pair.counter
            envelope = self.adapter.build_manage_sell_offer(
                source=wallet.address(),
                selling=selling,
                buying=buying,
                amount=amount,
                price=price,
                base_fee=base_fee_stroops,
                offer_id=offer_id,
            )
        fee_xlm = Decimal(base_fee_stroops) / STROOPS_PER_XLM
        review = OfferReview(
            wallet_name=wallet_name,
            source=wallet.address(),
            action=action,
            side=intent.side,
            base_asset=intent.pair.base.display,
            counter_asset=intent.pair.counter.display,
            amount=amount,
            price=price,
            total=_amount_text(intent.amount * intent.price),
            fee=_amount_text(fee_xlm),
            network=self.adapter.network.name,
            offer_id=str(offer_id) if offer_id else None,
        )
        return PreparedTransaction(envelope=envelope, review=review)

    def build_cancel_offer(
        self,
        wallet_name: str,
        wallet,
        offer: OpenOffer,
        base_fee_stroops: int,
    ) -> PreparedTransaction:
        envelope = self.adapter.build_manage_sell_offer(
            source=wallet.address(),
            selling=offer.selling,
            buying=offer.buying,
            amount="0",
            price=offer.price_r,
            base_fee=base_fee_stroops,
            offer_id=int(offer.offer_id),
        )
        fee_xlm = Decimal(base_fee_stroops) / STROOPS_PER_XLM
        review = OfferReview(
            wallet_name=wallet_name,
            source=wallet.address(),
            action="cancel",
            side=None,
            base_asset=offer.selling.display,
            counter_asset=offer.buying.display,
            amount=None,
            price=None,
            total=None,
            fee=_amount_text(fee_xlm),
            network=self.adapter.network.name,
            offer_id=offer.offer_id,
        )
        return PreparedTransaction(envelope=envelope, review=review)


def _amount_text(value: Decimal) -> str:
    text = format(value, "f")
    if "." in text:
        text = text.rstrip("0").rstrip(".")
    return text or "0"
