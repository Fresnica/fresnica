"""Balance, portfolio, and account-state service."""

from decimal import Decimal

from .availability import AvailabilityService
from .errors import FresnicaError
from .models import Asset, LiquidityPositionView, LiquidityReserveView


class BalanceService:
    def __init__(self, adapter, datastore, network_name: str):
        self.adapter = adapter
        self.datastore = datastore
        self.network_name = network_name
        self.availability = AvailabilityService()

    def get_account(self, wallet, refresh: bool = True) -> dict:
        address = wallet.address()
        if refresh:
            account = self.adapter.get_account(address)
            self.datastore.save_balances(
                self.network_name,
                address,
                account.get("balances", []),
            )
            return account

        return {
            "account_id": address,
            "balances": self.datastore.get_balances(self.network_name, address),
        }

    def get_balances(self, wallet, refresh: bool = True) -> list[dict]:
        return self.get_account(wallet, refresh=refresh).get("balances", [])

    def get_views(self, wallet):
        account = self.get_account(wallet, refresh=True)
        return self._views_for_account(account)

    def get_portfolio_views(self, wallet):
        account = self.get_account(wallet, refresh=True)
        views = self._views_for_account(account)
        balances = [item for item in views if not item.asset.is_liquidity_pool]
        positions = [
            self._liquidity_position(item)
            for item in views
            if item.asset.is_liquidity_pool and item.balance > 0
        ]
        return balances, positions

    def _views_for_account(self, account: dict):
        base_reserve = self.adapter.get_base_reserve_stroops()
        views = self.availability.balance_views(account, base_reserve)
        return sorted(views, key=_balance_sort_key)

    def _liquidity_position(self, balance_view) -> LiquidityPositionView:
        pool_id = balance_view.asset.liquidity_pool_id or ""
        shares = balance_view.balance
        if not pool_id:
            return LiquidityPositionView(
                pool_id="unknown",
                shares=shares,
                error="Liquidity pool id is missing from Horizon balance data",
                raw=balance_view.raw,
            )

        pool = None
        lookup_error = None
        try:
            pool = self.adapter.get_liquidity_pool(pool_id)
            self.datastore.save_liquidity_pool(self.network_name, pool_id, pool)
        except (FresnicaError, ValueError, ArithmeticError) as exc:
            lookup_error = exc
            pool = self.datastore.get_liquidity_pool(self.network_name, pool_id)

        if pool is None:
            return LiquidityPositionView(
                pool_id=pool_id,
                shares=shares,
                error=str(lookup_error) if lookup_error is not None else "Liquidity pool details unavailable",
                raw=balance_view.raw,
            )

        try:
            total_shares = Decimal(str(pool.get("total_shares", "0")))
            ratio = shares / total_shares if total_shares > 0 else Decimal("0")
            reserves = []
            for reserve in pool.get("reserves", []):
                asset = Asset.from_horizon_string(str(reserve.get("asset", "")))
                amount = Decimal(str(reserve.get("amount", "0"))) * ratio
                reserves.append(LiquidityReserveView(asset=asset, amount=amount))
            return LiquidityPositionView(
                pool_id=pool_id,
                shares=shares,
                reserves=reserves,
                raw=pool,
            )
        except (ValueError, ArithmeticError) as exc:
            return LiquidityPositionView(
                pool_id=pool_id,
                shares=shares,
                error=str(exc),
                raw=pool,
            )


def _balance_sort_key(item):
    asset = item.asset
    if asset.is_native:
        return (0, "", "")
    if asset.is_liquidity_pool:
        return (2, asset.liquidity_pool_id or "", "")
    return (1, asset.code.upper(), asset.issuer or "")
