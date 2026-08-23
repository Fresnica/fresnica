"""User-facing trustline lifecycle orchestration."""

from decimal import Decimal, InvalidOperation

from .availability import AvailabilityService, STROOPS_PER_XLM
from .errors import (
    InsufficientBalanceError,
    InvalidAmountError,
    TransactionError,
    WatchOnlyError,
)
from .models import Asset
from .trustline_policy import FRESNICA_TRUSTLINE_LIMIT


class TrustlineService:
    def __init__(self, balance_service, transaction_builder, transaction_service):
        self.balance_service = balance_service
        self.transaction_builder = transaction_builder
        self.transaction_service = transaction_service
        self.availability = AvailabilityService()

    def prepare_add(self, wallet_name: str, wallet, asset, limit=None):
        self._ensure_signing_wallet(wallet)
        asset = self._asset(asset, wallet)
        limit_value = (
            _limit(limit) if limit is not None else FRESNICA_TRUSTLINE_LIMIT
        )
        account, base_fee, base_reserve = self._account_context(wallet)
        if _find_trustline(account, asset) is not None:
            raise TransactionError(
                f"Trustline already exists for {_asset_identity(asset)}; use trust limit to change its limit"
            )
        self._ensure_native_capacity(
            account,
            base_reserve,
            base_fee,
            additional_reserve_stroops=base_reserve,
        )
        return self.transaction_builder.build_trustline(
            wallet_name=wallet_name,
            wallet=wallet,
            asset=asset,
            base_fee_stroops=base_fee,
            action="add",
            limit=limit_value,
        )

    def prepare_limit(self, wallet_name: str, wallet, asset, limit):
        self._ensure_signing_wallet(wallet)
        asset = self._asset(asset, wallet)
        limit_value = _limit(limit)
        account, base_fee, base_reserve = self._account_context(wallet)
        raw = _find_trustline(account, asset)
        if raw is None:
            raise TransactionError(
                f"Trustline does not exist for {_asset_identity(asset)}; use trust add first"
            )
        committed = Decimal(raw.get("balance", "0")) + Decimal(
            raw.get("buying_liabilities", "0")
        )
        if limit_value < committed:
            raise InvalidAmountError(
                f"Trustline limit cannot be below current balance plus buying liabilities ({committed})"
            )
        self._ensure_native_capacity(account, base_reserve, base_fee)
        return self.transaction_builder.build_trustline(
            wallet_name=wallet_name,
            wallet=wallet,
            asset=asset,
            base_fee_stroops=base_fee,
            action="limit",
            limit=limit_value,
        )

    def prepare_remove(self, wallet_name: str, wallet, asset):
        self._ensure_signing_wallet(wallet)
        asset = self._asset(asset, wallet)
        account, base_fee, base_reserve = self._account_context(wallet)
        raw = _find_trustline(account, asset)
        if raw is None:
            raise TransactionError(f"Trustline does not exist for {_asset_identity(asset)}")

        balance = Decimal(raw.get("balance", "0"))
        selling = Decimal(raw.get("selling_liabilities", "0"))
        buying = Decimal(raw.get("buying_liabilities", "0"))
        if balance != 0 or selling != 0 or buying != 0:
            raise TransactionError(
                "Trustline cannot be removed while balance or liabilities are non-zero"
            )
        self._ensure_native_capacity(account, base_reserve, base_fee)
        return self.transaction_builder.build_trustline(
            wallet_name=wallet_name,
            wallet=wallet,
            asset=asset,
            base_fee_stroops=base_fee,
            action="remove",
            limit=Decimal("0"),
        )

    def sign(self, wallet, prepared):
        return self.transaction_service.sign(wallet, prepared)

    def submit(self, prepared):
        return self.transaction_service.submit(prepared)

    def _account_context(self, wallet):
        account = self.balance_service.get_account(wallet, refresh=True)
        adapter = self.balance_service.adapter
        return (
            account,
            adapter.fetch_base_fee(),
            adapter.get_base_reserve_stroops(),
        )

    def _ensure_native_capacity(
        self,
        account: dict,
        base_reserve_stroops: int,
        fee_stroops: int,
        additional_reserve_stroops: int = 0,
    ) -> None:
        native = _find_native(account)
        if native is None:
            raise InsufficientBalanceError("XLM", "reserve and fee", "0")
        balance = Decimal(native.get("balance", "0"))
        selling = Decimal(native.get("selling_liabilities", "0"))
        free = (
            balance
            - selling
            - self.availability.minimum_balance_xlm(account, base_reserve_stroops)
        )
        required = Decimal(fee_stroops + additional_reserve_stroops) / STROOPS_PER_XLM
        if free < required:
            raise InsufficientBalanceError("XLM", required, max(free, Decimal("0")))

    @staticmethod
    def _ensure_signing_wallet(wallet) -> None:
        if not wallet.can_sign():
            raise WatchOnlyError("Watch-only wallet cannot change trustlines")

    @staticmethod
    def _asset(asset, wallet) -> Asset:
        asset = asset if isinstance(asset, Asset) else Asset.parse(asset)
        if asset.is_native:
            raise TransactionError("XLM is native and does not use a trustline")
        if asset.is_liquidity_pool:
            raise TransactionError("Liquidity pool shares are not trustline assets")
        if asset.issuer == wallet.address():
            raise TransactionError("An asset issuer cannot create a trustline to its own asset")
        return asset


def _limit(value) -> Decimal:
    try:
        limit = Decimal(str(value))
    except (InvalidOperation, ValueError) as exc:
        raise InvalidAmountError(f"Invalid trustline limit: {value}") from exc
    if not limit.is_finite() or limit <= 0:
        raise InvalidAmountError("Trustline limit must be greater than zero")
    if limit.as_tuple().exponent < -7:
        raise InvalidAmountError("Stellar amounts support at most 7 decimal places")
    return limit


def _find_trustline(account: dict, asset: Asset) -> dict | None:
    for raw in account.get("balances", []):
        if raw.get("asset_type") == "native":
            continue
        if raw.get("asset_type") == "liquidity_pool_shares":
            continue
        if raw.get("asset_code") == asset.code and raw.get("asset_issuer") == asset.issuer:
            return raw
    return None


def _find_native(account: dict) -> dict | None:
    for raw in account.get("balances", []):
        if raw.get("asset_type") == "native":
            return raw
    return None


def _asset_identity(asset: Asset) -> str:
    return f"{asset.code}:{asset.issuer}"
