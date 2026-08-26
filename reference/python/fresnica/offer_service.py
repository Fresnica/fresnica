"""SDEX offer domain helpers and write orchestration."""

from decimal import Decimal, InvalidOperation, ROUND_HALF_UP

from .errors import (
    InsufficientBalanceError,
    InvalidAmountError,
    TransactionError,
    TrustlineConfirmationRequired,
    WatchOnlyError,
)
from .models import Asset, MarketPair, OfferIntent, OfferView, OpenOffer, PriceRatio


STROOP_SCALE = 10_000_000
STELLAR_QUANTUM = Decimal("0.0000001")
INT32_MAX = 2_147_483_647


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

    def prepare_create(
        self,
        wallet_name: str,
        wallet,
        intent: OfferIntent,
        allow_trustline: bool = False,
    ):
        self._validate_wallet(wallet)
        intent = _validated_intent(intent)
        price_r = _stellar_price_ratio(intent.price)
        adapter = self.transaction_builder.adapter
        fee = adapter.fetch_base_fee()
        account = adapter.get_account(wallet.address())
        buying = _buying_asset(intent)
        adds_trustline = not _account_can_hold(account, buying)
        operation_count = 2 if adds_trustline else 1
        _preflight_new_offer(
            account,
            intent,
            price_r,
            base_reserve_stroops=adapter.get_base_reserve_stroops(),
            fee_stroops=fee * operation_count,
            extra_subentries=1 + (1 if adds_trustline else 0),
        )
        if adds_trustline and not allow_trustline:
            raise TrustlineConfirmationRequired(buying.display)
        return self.transaction_builder.build_offer(
            wallet_name=wallet_name,
            wallet=wallet,
            intent=intent,
            price_r=price_r,
            base_fee_stroops=fee,
            action="create",
            trustline_asset=buying if adds_trustline else None,
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
        price_r = _stellar_price_ratio(intent.price)
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
            price_r=price_r,
            base_fee_stroops=fee,
            offer_id=int(offer.offer_id),
            action="update",
        )

    def prepare_cancel(
        self,
        wallet_name: str,
        wallet,
        offer: OpenOffer,
        pair: MarketPair | None = None,
    ):
        self._validate_wallet(wallet)
        view = offer_view_for_pair(offer, pair) if pair is not None else None
        if pair is not None and view is None:
            raise TransactionError("Offer cancellation pair does not match the current offer")
        fee = self.transaction_builder.adapter.fetch_base_fee()
        return self.transaction_builder.build_cancel_offer(
            wallet_name=wallet_name,
            wallet=wallet,
            offer=offer,
            base_fee_stroops=fee,
            view=view,
        )

    def sign(self, wallet, prepared):
        return self.transaction_service.sign(wallet, prepared)

    def submit(self, prepared):
        return self.transaction_service.submit(prepared)

    @staticmethod
    def _validate_wallet(wallet) -> None:
        if not wallet.can_sign():
            raise WatchOnlyError("Watch-only wallet cannot manage offers")


def _preflight_new_offer(
    account: dict,
    intent: OfferIntent,
    price_r: PriceRatio,
    base_reserve_stroops: int,
    fee_stroops: int,
    extra_subentries: int,
) -> None:
    selling = _selling_asset(intent)
    required_selling = _required_selling_stroops(intent, price_r)
    account_id = str(account.get("account_id", ""))

    # Issuers can sell newly issued units of their own asset without a trustline.
    if selling.is_native or selling.issuer != account_id:
        selling_balance = _find_balance(account, selling)
        available_selling = _available_balance_stroops(selling_balance)
        if selling.is_native:
            reserve_units = max(
                2
                + int(account.get("subentry_count", 0))
                + int(account.get("num_sponsoring", 0))
                - int(account.get("num_sponsored", 0))
                + extra_subentries,
                0,
            )
            required_selling += reserve_units * base_reserve_stroops + fee_stroops
        if required_selling > available_selling:
            raise InsufficientBalanceError(
                selling.display,
                _from_stroops(required_selling),
                _from_stroops(available_selling),
            )

    # A new offer always consumes one reserve subentry, even when selling a
    # credit asset. A confirmed new receiving trustline consumes one more.
    if not selling.is_native:
        native = _find_balance(account, Asset("XLM"))
        available_native = _available_balance_stroops(native)
        reserve_units = max(
            2
            + int(account.get("subentry_count", 0))
            + int(account.get("num_sponsoring", 0))
            - int(account.get("num_sponsored", 0))
            + extra_subentries,
            0,
        )
        required_native = reserve_units * base_reserve_stroops + fee_stroops
        if required_native > available_native:
            raise InsufficientBalanceError(
                "XLM",
                _from_stroops(required_native),
                _from_stroops(available_native),
            )


def _required_selling_stroops(intent: OfferIntent, price_r: PriceRatio) -> int:
    amount = _to_stroops(intent.amount)
    if intent.side == "sell":
        return amount
    return (amount * price_r.n + price_r.d - 1) // price_r.d


def _stellar_price_ratio(price: Decimal) -> PriceRatio:
    numerator = _to_stroops(price)
    denominator = STROOP_SCALE
    previous_n, previous_d = 0, 1
    current_n, current_d = 1, 0
    best_n, best_d = current_n, current_d

    while True:
        if numerator > denominator * INT32_MAX:
            break
        coefficient = numerator // denominator
        next_n = coefficient * current_n + previous_n
        next_d = coefficient * current_d + previous_d
        if next_n > INT32_MAX or next_d > INT32_MAX:
            break
        best_n, best_d = next_n, next_d
        previous_n, previous_d = current_n, current_d
        current_n, current_d = next_n, next_d
        remainder = numerator % denominator
        if remainder == 0:
            break
        numerator, denominator = denominator, remainder

    if best_n <= 0 or best_d <= 0:
        coefficient = INT32_MAX
        if current_n > 0:
            coefficient = min(coefficient, (INT32_MAX - previous_n) // current_n)
        if current_d > 0:
            coefficient = min(coefficient, (INT32_MAX - previous_d) // current_d)
        if coefficient >= 1:
            recovered_n = coefficient * current_n + previous_n
            recovered_d = coefficient * current_d + previous_d
            if 0 < recovered_n <= INT32_MAX and 0 < recovered_d <= INT32_MAX:
                best_n, best_d = recovered_n, recovered_d

    if best_n <= 0 or best_d <= 0:
        raise InvalidAmountError("Offer price has no Stellar int32 rational approximation")
    return PriceRatio(best_n, best_d)


def _account_can_hold(account: dict, asset: Asset) -> bool:
    if asset.is_native:
        return True
    if asset.issuer == account.get("account_id"):
        return True
    return _find_balance(account, asset) is not None


def _find_balance(account: dict, asset: Asset) -> dict | None:
    for raw in account.get("balances", []):
        if asset.is_native:
            if raw.get("asset_type") == "native":
                return raw
        elif (
            raw.get("asset_code") == asset.code
            and raw.get("asset_issuer") == asset.issuer
        ):
            return raw
    return None


def _available_balance_stroops(raw: dict | None) -> int:
    if raw is None:
        return 0
    balance = _decimal_stroops(raw.get("balance", "0"))
    liabilities = _decimal_stroops(raw.get("selling_liabilities", "0"))
    return max(balance - liabilities, 0)


def _decimal_stroops(value) -> int:
    try:
        return int(Decimal(str(value)) * STROOP_SCALE)
    except (InvalidOperation, ValueError, TypeError):
        return 0


def _to_stroops(value: Decimal) -> int:
    return int(value * STROOP_SCALE)


def _from_stroops(value: int) -> Decimal:
    return Decimal(value) / Decimal(STROOP_SCALE)


def _selling_asset(intent: OfferIntent) -> Asset:
    return intent.pair.counter if intent.side == "buy" else intent.pair.base


def _buying_asset(intent: OfferIntent) -> Asset:
    return intent.pair.base if intent.side == "buy" else intent.pair.counter


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
