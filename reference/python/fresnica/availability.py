"""Spendable-balance calculations from raw Horizon account state."""

from decimal import Decimal, InvalidOperation

from .errors import InvalidAmountError, InsufficientBalanceError, TransactionError
from .models import Asset, BalanceView


STROOPS_PER_XLM = Decimal("10000000")
MAX_STELLAR_AMOUNT = Decimal(2**63 - 1) / STROOPS_PER_XLM


class AvailabilityService:
    def minimum_balance_xlm(
        self,
        account: dict,
        base_reserve_stroops: int,
    ) -> Decimal:
        reserve_units = (
            2
            + int(account.get("subentry_count", 0))
            + int(account.get("num_sponsoring", 0))
            - int(account.get("num_sponsored", 0))
        )
        reserve_units = max(reserve_units, 0)
        base_reserve = Decimal(base_reserve_stroops) / STROOPS_PER_XLM
        return Decimal(reserve_units) * base_reserve

    def balance_views(
        self,
        account: dict,
        base_reserve_stroops: int,
    ) -> list[BalanceView]:
        views = []
        for raw in account.get("balances", []):
            asset = _asset_from_balance(raw)
            balance = Decimal(raw.get("balance", "0"))
            selling = Decimal(raw.get("selling_liabilities", "0"))
            buying = Decimal(raw.get("buying_liabilities", "0"))
            if asset.is_liquidity_pool:
                available = balance
                receiving_capacity = None
            elif asset.is_native:
                available = (
                    balance
                    - selling
                    - self.minimum_balance_xlm(account, base_reserve_stroops)
                )
                receiving_capacity = MAX_STELLAR_AMOUNT - balance - buying
            else:
                available = balance - selling
                receiving_capacity = Decimal(str(raw.get("limit", "0"))) - balance - buying
            views.append(
                BalanceView(
                    asset=asset,
                    balance=balance,
                    selling_liabilities=selling,
                    buying_liabilities=buying,
                    available=max(available, Decimal("0")),
                    receiving_capacity=(
                        None
                        if receiving_capacity is None
                        else max(receiving_capacity, Decimal("0"))
                    ),
                    raw=raw,
                )
            )
        return views

    def available_for_transfer(
        self,
        account: dict,
        asset: Asset,
        base_reserve_stroops: int,
        fee_stroops: int,
    ) -> Decimal:
        if not asset.is_native and asset.issuer == account.get("account_id"):
            return MAX_STELLAR_AMOUNT
        raw = _find_balance(account, asset)
        if raw is None:
            return Decimal("0")

        balance = Decimal(raw.get("balance", "0"))
        selling = Decimal(raw.get("selling_liabilities", "0"))
        if not asset.is_native:
            return max(balance - selling, Decimal("0"))

        fee = Decimal(fee_stroops) / STROOPS_PER_XLM
        available = (
            balance
            - selling
            - self.minimum_balance_xlm(account, base_reserve_stroops)
            - fee
        )
        return max(available, Decimal("0"))

    def validate_transfer(
        self,
        account: dict,
        asset: Asset,
        amount,
        base_reserve_stroops: int,
        fee_stroops: int,
    ) -> Decimal:
        requested = _amount(amount)
        if not asset.is_native and asset.issuer != account.get("account_id"):
            raw = _find_balance(account, asset)
            _ensure_full_authorization(raw, asset, "source")
        available = self.available_for_transfer(
            account,
            asset,
            base_reserve_stroops,
            fee_stroops,
        )
        if requested > available:
            raise InsufficientBalanceError(asset.display, requested, available)

        # Issued-asset payments still need enough free XLM for the transaction fee.
        if not asset.is_native:
            native = _find_balance(account, Asset("XLM"))
            if native is None:
                raise InsufficientBalanceError("XLM", "transaction fee", "0")
            fee = Decimal(fee_stroops) / STROOPS_PER_XLM
            native_balance = Decimal(native.get("balance", "0"))
            native_selling = Decimal(native.get("selling_liabilities", "0"))
            free_before_fee = (
                native_balance
                - native_selling
                - self.minimum_balance_xlm(account, base_reserve_stroops)
            )
            if free_before_fee < fee:
                raise InsufficientBalanceError(
                    "XLM", fee, max(free_before_fee, Decimal("0"))
                )
        return requested


    def receiving_capacity(self, account: dict, asset: Asset) -> Decimal:
        if not asset.is_native and asset.issuer == account.get("account_id"):
            return MAX_STELLAR_AMOUNT
        raw = _find_balance(account, asset)
        if raw is None:
            return Decimal("0")
        balance = Decimal(raw.get("balance", "0"))
        buying = Decimal(raw.get("buying_liabilities", "0"))
        if asset.is_native:
            return max(MAX_STELLAR_AMOUNT - balance - buying, Decimal("0"))
        limit = Decimal(raw.get("limit", "0"))
        return max(limit - balance - buying, Decimal("0"))

    def validate_receive(self, account: dict, asset: Asset, amount) -> Decimal:
        requested = _amount(amount)
        if not asset.is_native and asset.issuer != account.get("account_id"):
            raw = _find_balance(account, asset)
            _ensure_full_authorization(raw, asset, "destination")
        capacity = self.receiving_capacity(account, asset)
        if requested > capacity:
            raise InsufficientBalanceError(asset.display, requested, capacity)
        return requested


def _ensure_full_authorization(raw: dict | None, asset: Asset, role: str) -> None:
    if raw is None:
        raise TransactionError(f"{role.title()} trustline is missing for {asset.display}")
    authorized = raw.get("is_authorized")
    if authorized is True:
        return
    if authorized is False:
        raise TransactionError(
            f"{role.title()} trustline for {asset.display} is not fully authorized"
        )
    raise TransactionError(
        f"Horizon returned malformed authorization state for {asset.display}"
    )


def _amount(value) -> Decimal:
    try:
        amount = Decimal(str(value))
    except (InvalidOperation, ValueError) as exc:
        raise InvalidAmountError(f"Invalid amount: {value}") from exc
    if not amount.is_finite() or amount <= 0:
        raise InvalidAmountError("Amount must be greater than zero")
    if amount.as_tuple().exponent < -7:
        raise InvalidAmountError("Stellar amounts support at most 7 decimal places")
    return amount


def _asset_from_balance(raw: dict) -> Asset:
    asset_type = raw.get("asset_type")
    if asset_type == "native":
        return Asset("XLM")
    if asset_type == "liquidity_pool_shares":
        return Asset(
            "LP",
            liquidity_pool_id=raw.get("liquidity_pool_id"),
        )
    return Asset(raw.get("asset_code", ""), raw.get("asset_issuer"))


def _find_balance(account: dict, asset: Asset) -> dict | None:
    for raw in account.get("balances", []):
        candidate = _asset_from_balance(raw)
        if candidate == asset:
            return raw
    return None
