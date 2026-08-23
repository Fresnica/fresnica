"""SDEX offer domain helpers and write orchestration."""

from decimal import Decimal, InvalidOperation, ROUND_HALF_UP

from .errors import InvalidAmountError, TransactionError, WatchOnlyError
from .models import Asset, MarketPair, OfferIntent, OfferView, OpenOffer, PriceRatio


STELLAR_QUANTUM = Decimal("0.0000001")


def open_offer_from_horizon(raw: dict) -> OpenOffer:
    price_r = raw.get("price_r") or {}
    try:
        ratio = PriceRatio(int(price_r["n"]), int(price_r["d"]))
        amount = Decimal(str(raw["amount"]))
    except (KeyError, TypeError, ValueError, InvalidOperation) as exc:
        raise ValueError("Invalid Horizon offer record") from exc
    return OpenOffer(
        offer_id=str(raw.get("id", raw.get("paging_token", ""))),
        seller=raw.get("seller"),
        selling=_asset_from_horizon(raw.get("selling", {})),
        buying=_asset_from_horizon(raw.get("buying", {})),
        selling_amount=amount,
        price_r=ratio,
        last_modified_ledger=raw.get("last_modified_ledger"),
        last_modified_time=raw.get("last_modified_time"),
        raw=raw,
    )


def offer_view_for_pair(offer: OpenOffer, pair: MarketPair) -> OfferView | None:
    ratio = Decimal(offer.price_r.n) / Decimal(offer.price_r.d)
    if offer.selling == pair.base and offer.buying == pair.counter:
        return OfferView(
            pair=pair,
            side="sell",
            amount=offer.selling_amount,
            price=_stellar_round(ratio),
            total=_stellar_round(offer.selling_amount * ratio),
        )

    if offer.selling == pair.counter and offer.buying == pair.base:
        # Horizon stores remaining selling amount, not the original buy intent.
        # This is a user-facing projection and must not be treated as reversible
        # historical intent after partial fills.
        return OfferView(
            pair=pair,
            side="buy",
            amount=_stellar_round(offer.selling_amount * ratio),
            price=_stellar_round(Decimal(offer.price_r.d) / Decimal(offer.price_r.n)),
            total=offer.selling_amount,
        )

    return None


def canonical_operation(intent: OfferIntent) -> str:
    return "manage_buy_offer" if intent.side == "buy" else "manage_sell_offer"


class OfferService:
    def __init__(self, transaction_builder, transaction_service):
        self.transaction_builder = transaction_builder
        self.transaction_service = transaction_service

    def prepare_create(self, wallet_name: str, wallet, intent: OfferIntent):
        self._validate_wallet(wallet)
        intent = _validated_intent(intent)
        fee = self.transaction_builder.adapter.fetch_base_fee()
        return self.transaction_builder.build_offer(
            wallet_name=wallet_name,
            wallet=wallet,
            intent=intent,
            base_fee_stroops=fee,
            action="create",
        )

    def prepare_update(
        self,
        wallet_name: str,
        wallet,
        offer: OpenOffer,
        intent: OfferIntent,
    ):
        self._validate_wallet(wallet)
        intent = _validated_intent(intent)
        current_view = offer_view_for_pair(offer, intent.pair)
        if current_view is None or current_view.side != intent.side:
            raise TransactionError(
                "Offer update must keep the current market pair and BUY/SELL side"
            )
        fee = self.transaction_builder.adapter.fetch_base_fee()
        return self.transaction_builder.build_offer(
            wallet_name=wallet_name,
            wallet=wallet,
            intent=intent,
            base_fee_stroops=fee,
            offer_id=int(offer.offer_id),
            action="update",
        )

    def prepare_cancel(self, wallet_name: str, wallet, offer: OpenOffer):
        self._validate_wallet(wallet)
        fee = self.transaction_builder.adapter.fetch_base_fee()
        return self.transaction_builder.build_cancel_offer(
            wallet_name=wallet_name,
            wallet=wallet,
            offer=offer,
            base_fee_stroops=fee,
        )

    def sign(self, wallet, prepared):
        return self.transaction_service.sign(wallet, prepared)

    def submit(self, prepared):
        return self.transaction_service.submit(prepared)

    @staticmethod
    def _validate_wallet(wallet) -> None:
        if not wallet.can_sign():
            raise WatchOnlyError("Watch-only wallet cannot manage offers")


def _validated_intent(intent: OfferIntent) -> OfferIntent:
    if intent.pair.base == intent.pair.counter:
        raise TransactionError("Offer assets must be different")
    if intent.side not in ("buy", "sell"):
        raise TransactionError(f"Unsupported offer side: {intent.side}")
    amount = _stellar_value(intent.amount, "amount")
    price = _stellar_value(intent.price, "price")
    return OfferIntent(
        pair=intent.pair,
        side=intent.side,
        amount=amount,
        price=price,
    )


def _stellar_value(value, label: str) -> Decimal:
    try:
        decimal = Decimal(str(value))
    except (InvalidOperation, ValueError) as exc:
        raise InvalidAmountError(f"Invalid offer {label}: {value}") from exc
    if not decimal.is_finite() or decimal <= 0:
        raise InvalidAmountError(f"Offer {label} must be greater than zero")
    if decimal.as_tuple().exponent < -7:
        raise InvalidAmountError(f"Offer {label} supports at most 7 decimal places")
    return decimal


def _stellar_round(value: Decimal) -> Decimal:
    return value.quantize(STELLAR_QUANTUM, rounding=ROUND_HALF_UP)


def _asset_from_horizon(raw: dict) -> Asset:
    if raw.get("asset_type") == "native":
        return Asset("XLM")
    code = raw.get("asset_code")
    issuer = raw.get("asset_issuer")
    if not code or not issuer:
        raise ValueError("Invalid Horizon offer asset")
    return Asset(str(code), str(issuer))
