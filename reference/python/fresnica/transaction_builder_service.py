"""Turn wallet-level transaction intent into an SDK transaction envelope."""

from dataclasses import dataclass
from decimal import Decimal

from .availability import STROOPS_PER_XLM
from .models import Asset, OfferIntent, OfferView, OpenOffer, PriceRatio
from .review import OfferReview, TransactionReview, TrustlineReview
from .trustline_policy import FRESNICA_TRUSTLINE_LIMIT_TEXT


@dataclass
class PreparedTransaction:
    envelope: object
    review: TransactionReview | OfferReview | TrustlineReview


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
        memo_type: str | None = None,
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
            memo_type=memo_type,
            create_destination=create_destination,
        )
        fee_xlm = Decimal(base_fee_stroops) / STROOPS_PER_XLM
        review = TransactionReview(
            wallet_name=wallet_name,
            source=wallet.address(),
            destination=destination,
            asset=_review_asset(asset),
            amount=_amount_text(amount),
            fee=_amount_text(fee_xlm),
            network=self.adapter.network.name,
            operation="create_account" if create_destination else "payment",
            memo=memo,
            memo_type=memo_type,
            contact_name=contact_name,
        )
        return PreparedTransaction(envelope=envelope, review=review)

    def build_trustline(
        self,
        wallet_name: str,
        wallet,
        asset: Asset,
        base_fee_stroops: int,
        action: str,
        limit: Decimal | None = None,
        authorization: str | None = None,
        clawback_enabled: bool | None = None,
    ) -> PreparedTransaction:
        operation_limit = (
            _amount_text(limit)
            if limit is not None
            else FRESNICA_TRUSTLINE_LIMIT_TEXT if action == "add" else None
        )
        envelope = self.adapter.build_change_trust(
            source=wallet.address(),
            asset=asset,
            limit=operation_limit,
            base_fee=base_fee_stroops,
        )
        if action == "remove":
            review_limit = None
        else:
            review_limit = operation_limit
        fee_xlm = Decimal(base_fee_stroops) / STROOPS_PER_XLM
        review = TrustlineReview(
            wallet_name=wallet_name,
            source=wallet.address(),
            action=action,
            asset=_review_asset(asset),
            limit=review_limit,
            fee=_amount_text(fee_xlm),
            network=self.adapter.network.name,
            authorization=authorization,
            clawback_enabled=clawback_enabled,
        )
        return PreparedTransaction(envelope=envelope, review=review)

    def build_offer(
        self,
        wallet_name: str,
        wallet,
        intent: OfferIntent,
        price_r: PriceRatio,
        base_fee_stroops: int,
        offer_id: int = 0,
        action: str = "create",
        trustline_asset: Asset | None = None,
    ) -> PreparedTransaction:
        amount = _amount_text(intent.amount)
        requested_price = _amount_text(intent.price)
        effective_price = Decimal(price_r.n) / Decimal(price_r.d)
        price = _amount_text(effective_price)
        if intent.side == "buy":
            selling = intent.pair.counter
            buying = intent.pair.base
            envelope = self.adapter.build_manage_buy_offer(
                source=wallet.address(),
                selling=selling,
                buying=buying,
                buy_amount=amount,
                price=price_r,
                base_fee=base_fee_stroops,
                offer_id=offer_id,
                trustline_asset=trustline_asset,
            )
        else:
            selling = intent.pair.base
            buying = intent.pair.counter
            envelope = self.adapter.build_manage_sell_offer(
                source=wallet.address(),
                selling=selling,
                buying=buying,
                amount=amount,
                price=price_r,
                base_fee=base_fee_stroops,
                offer_id=offer_id,
                trustline_asset=trustline_asset,
            )
        operation_count = 2 if trustline_asset is not None else 1
        fee_xlm = Decimal(base_fee_stroops * operation_count) / STROOPS_PER_XLM
        review = OfferReview(
            wallet_name=wallet_name,
            source=wallet.address(),
            action=action,
            side=intent.side,
            base_asset=_review_asset(intent.pair.base),
            counter_asset=_review_asset(intent.pair.counter),
            amount=amount,
            price=price,
            total=_amount_text(intent.amount * effective_price),
            fee=_amount_text(fee_xlm),
            network=self.adapter.network.name,
            offer_id=str(offer_id) if offer_id else None,
            requested_price=(requested_price if effective_price != intent.price else None),
            price_n=price_r.n,
            price_d=price_r.d,
            trustline_asset=_review_asset(trustline_asset) if trustline_asset else None,
            trustline_limit=(
                FRESNICA_TRUSTLINE_LIMIT_TEXT if trustline_asset is not None else None
            ),
        )
        return PreparedTransaction(envelope=envelope, review=review)

    def build_cancel_offer(
        self,
        wallet_name: str,
        wallet,
        offer: OpenOffer,
        base_fee_stroops: int,
        view: OfferView | None = None,
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
        if view is None:
            side = None
            base_asset = _review_asset(offer.selling)
            counter_asset = _review_asset(offer.buying)
            amount = price = total = None
        else:
            side = view.side
            base_asset = _review_asset(view.pair.base)
            counter_asset = _review_asset(view.pair.counter)
            amount = _amount_text(view.amount)
            price = _amount_text(view.price)
            total = _amount_text(view.total)
        review = OfferReview(
            wallet_name=wallet_name,
            source=wallet.address(),
            action="cancel",
            side=side,
            base_asset=base_asset,
            counter_asset=counter_asset,
            amount=amount,
            price=price,
            total=total,
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


def _review_asset(asset: Asset) -> str:
    if asset.is_native:
        return "XLM"
    if asset.is_liquidity_pool:
        return asset.display
    return f"{asset.code}:{asset.issuer}"
