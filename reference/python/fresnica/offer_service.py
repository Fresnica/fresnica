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
from .trustline_policy import FRESNICA_TRUSTLINE_LIMIT


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
        selling = _selling_asset(intent)
        buying = _buying_asset(intent)
        adds_trustline = not _account_can_hold(account, buying)
        _ensure_full_offer_authorization(account, selling)
        if not adds_trustline:
            _ensure_full_offer_authorization(account, buying)
        if adds_trustline and allow_trustline:
            issuer_account = adapter.get_account(buying.issuer)
            if _issuer_requires_authorization(issuer_account):
                raise TransactionError(
                    f"Receiving trustline for {buying.display} requires issuer authorization "
                    "before an offer can be created"
                )
        operation_count = 2 if adds_trustline else 1
        _preflight_new_offer(
            account,
            intent,
            price_r,
            base_reserve_stroops=adapter.get_base_reserve_stroops(),
            fee_stroops=fee * operation_count,
            extra_subentries=1 + (1 if adds_trustline else 0),
            adds_trustline=adds_trustline,
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
        adapter = self.transaction_builder.adapter
        fee = adapter.fetch_base_fee()
        account = adapter.get_account(wallet.address())
        _ensure_full_offer_authorization(account, _selling_asset(intent))
        _ensure_full_offer_authorization(account, _buying_asset(intent))
        _preflight_update_offer(
            account,
            intent,
            price_r,
            offer,
            base_reserve_stroops=adapter.get_base_reserve_stroops(),
            fee_stroops=fee,
        )
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
        adapter = self.transaction_builder.adapter
        fee = adapter.fetch_base_fee()
        account = adapter.get_account(wallet.address())
        _ensure_offer_fee_capacity(account, adapter.get_base_reserve_stroops(), fee)
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
    adds_trustline: bool,
) -> None:
    selling = _selling_asset(intent)
    buying = _buying_asset(intent)
    liabilities = _offer_liabilities(intent.side, _to_stroops(intent.amount), price_r)
    account_id = str(account.get("account_id", ""))

    # Issuers can sell newly issued units of their own asset without a trustline.
    if selling.is_native or selling.issuer != account_id:
        selling_balance = _find_balance(account, selling)
        available_selling = _available_balance_stroops(selling_balance)
        required_selling = liabilities[0]
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

    new_limit = _to_stroops(FRESNICA_TRUSTLINE_LIMIT) if adds_trustline else None
    receive_capacity = _receiving_capacity_stroops(account, buying, account_id, new_limit)
    if liabilities[1] > receive_capacity:
        raise InsufficientBalanceError(
            buying.display,
            _from_stroops(liabilities[1]),
            _from_stroops(receive_capacity),
        )


def _preflight_update_offer(
    account: dict,
    intent: OfferIntent,
    price_r: PriceRatio,
    offer: OpenOffer,
    base_reserve_stroops: int,
    fee_stroops: int,
) -> None:
    selling = _selling_asset(intent)
    buying = _buying_asset(intent)
    account_id = str(account.get("account_id", ""))
    _ensure_offer_fee_capacity(account, base_reserve_stroops, fee_stroops)
    new_liabilities = _offer_liabilities(intent.side, _to_stroops(intent.amount), price_r)
    old_liabilities = _offer_liabilities(
        "sell", _to_stroops(offer.selling_amount), offer.price_r
    )

    if selling.is_native or selling.issuer != account_id:
        available_after_release = (
            _available_balance_stroops(_find_balance(account, selling))
            + old_liabilities[0]
        )
        required_selling = new_liabilities[0]
        if selling.is_native:
            reserve_units = max(
                2
                + int(account.get("subentry_count", 0))
                + int(account.get("num_sponsoring", 0))
                - int(account.get("num_sponsored", 0)),
                0,
            )
            required_selling += reserve_units * base_reserve_stroops + fee_stroops
        if required_selling > available_after_release:
            raise InsufficientBalanceError(
                selling.display,
                _from_stroops(required_selling),
                _from_stroops(available_after_release),
            )


    if buying.issuer == account_id:
        receive_after_release = 2**63 - 1
    else:
        receive_after_release = (
            _receiving_capacity_stroops(account, buying, account_id)
            + old_liabilities[1]
        )
    if new_liabilities[1] > receive_after_release:
        raise InsufficientBalanceError(
            buying.display,
            _from_stroops(new_liabilities[1]),
            _from_stroops(receive_after_release),
        )


def _ensure_offer_fee_capacity(
    account: dict, base_reserve_stroops: int, fee_stroops: int
) -> None:
    native = _find_balance(account, Asset("XLM"))
    free = max(
        _available_balance_stroops(native)
        - max(
            2
            + int(account.get("subentry_count", 0))
            + int(account.get("num_sponsoring", 0))
            - int(account.get("num_sponsored", 0)),
            0,
        )
        * base_reserve_stroops,
        0,
    )
    if free < fee_stroops:
        raise InsufficientBalanceError(
            "XLM", _from_stroops(fee_stroops), _from_stroops(free)
        )


def _offer_liabilities(side: str, amount: int, price_r: PriceRatio) -> tuple[int, int]:
    if side == "sell":
        price_n, price_d = price_r.n, price_r.d
        max_wheat_send, max_sheep_receive = amount, 2**63 - 1
    elif side == "buy":
        price_n, price_d = price_r.d, price_r.n
        max_wheat_send, max_sheep_receive = 2**63 - 1, amount
    else:
        raise TransactionError(f"Unsupported offer side: {side}")
    return _exchange_v10_normal_liabilities(
        price_n, price_d, max_wheat_send, max_sheep_receive
    )


def _exchange_v10_normal_liabilities(
    price_n: int, price_d: int, max_wheat_send: int, max_sheep_receive: int
) -> tuple[int, int]:
    if price_n <= 0 or price_d <= 0 or max_wheat_send < 0 or max_sheep_receive < 0:
        raise TransactionError("Invalid offer liability inputs")
    int64_max = 2**63 - 1
    wheat_value = min(max_wheat_send * price_n, max_sheep_receive * price_d)
    sheep_value = min(int64_max * price_d, int64_max * price_n)
    wheat_stays = wheat_value > sheep_value

    if wheat_stays:
        if price_n > price_d:
            wheat_receive = sheep_value // price_n
            sheep_send = _ceil_div(wheat_receive * price_n, price_d)
        else:
            sheep_send = sheep_value // price_d
            wheat_receive = sheep_send * price_d // price_n
    elif price_n > price_d:
        wheat_receive = wheat_value // price_n
        sheep_send = wheat_receive * price_n // price_d
    else:
        sheep_send = wheat_value // price_d
        wheat_receive = _ceil_div(sheep_send * price_d, price_n)
    return wheat_receive, sheep_send


def _ceil_div(numerator: int, denominator: int) -> int:
    if numerator < 0 or denominator <= 0:
        raise TransactionError("Invalid offer liability division")
    return (numerator + denominator - 1) // denominator


def _receiving_capacity_stroops(
    account: dict, asset: Asset, account_id: str, new_trustline_limit: int | None = None
) -> int:
    if not asset.is_native and asset.issuer == account_id:
        return 2**63 - 1
    raw = _find_balance(account, asset)
    if asset.is_native:
        if raw is None:
            raise TransactionError("Horizon returned no native balance")
        return max(
            (2**63 - 1)
            - _decimal_stroops(raw.get("balance", "0"))
            - _decimal_stroops(raw.get("buying_liabilities", "0")),
            0,
        )
    if raw is None:
        return new_trustline_limit or 0
    return max(
        _decimal_stroops(raw.get("limit", "0"))
        - _decimal_stroops(raw.get("balance", "0"))
        - _decimal_stroops(raw.get("buying_liabilities", "0")),
        0,
    )


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


def _ensure_full_offer_authorization(account: dict, asset: Asset) -> None:
    account_id = str(account.get("account_id", ""))
    if asset.is_native or asset.issuer == account_id:
        return
    raw = _find_balance(account, asset)
    if raw is None:
        raise TransactionError(f"Trustline is missing for {asset.display}")
    authorized = raw.get("is_authorized")
    if authorized is True:
        return
    if authorized is False:
        raise TransactionError(
            f"Trustline for {asset.display} is not fully authorized for offer management"
        )
    raise TransactionError(
        f"Horizon returned malformed authorization state for {asset.display}"
    )


def _issuer_requires_authorization(account: dict) -> bool:
    flags = account.get("flags")
    if not isinstance(flags, dict) or not isinstance(flags.get("auth_required"), bool):
        raise TransactionError("Horizon returned malformed issuer authorization flags")
    return flags["auth_required"]


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
