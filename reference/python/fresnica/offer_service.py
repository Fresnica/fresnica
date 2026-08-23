"""SDEX offer domain helpers shared by read and write workflows."""

from decimal import Decimal

from .models import MarketPair, OfferIntent, OfferView, OpenOffer, PriceRatio


def offer_view_for_pair(offer: OpenOffer, pair: MarketPair) -> OfferView | None:
    if offer.selling == pair.base and offer.buying == pair.counter:
        return OfferView(
            pair=pair,
            side="sell",
            amount=offer.selling_amount,
            price=Decimal(offer.price_r.n) / Decimal(offer.price_r.d),
            total=offer.selling_amount * Decimal(offer.price_r.n) / Decimal(offer.price_r.d),
        )

    if offer.selling == pair.counter and offer.buying == pair.base:
        return OfferView(
            pair=pair,
            side="buy",
            amount=offer.selling_amount * Decimal(offer.price_r.n) / Decimal(offer.price_r.d),
            price=Decimal(offer.price_r.d) / Decimal(offer.price_r.n),
            total=offer.selling_amount,
        )

    return None


def intent_price_ratio(intent: OfferIntent) -> PriceRatio:
    scale = 10_000_000
    return PriceRatio(
        n=int(intent.price * scale),
        d=scale,
    )


def canonical_operation(intent: OfferIntent) -> str:
    return "manage_buy_offer" if intent.side == "buy" else "manage_sell_offer"
