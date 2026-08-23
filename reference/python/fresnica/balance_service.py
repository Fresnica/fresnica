"""Balance, portfolio, and account-state service."""

from decimal import Decimal

from .availability import AvailabilityService
from .errors import FresnicaError
from .models import Asset, BalanceView, LiquidityPositionView, LiquidityReserveView


ISSUER_DOMAIN_CACHE_KEY = "_fresnica_issuer_domain"


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
            balances = account.get("balances", [])
            self._restore_cached_issuer_domains(address, balances)
            self._enrich_issuer_domains(balances)
            self.datastore.save_balances(
                self.network_name,
                address,
                balances,
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
        views = self._views_for_account(account)
        self._cache_views(wallet.address(), account, views)
        return views

    def get_portfolio_views(self, wallet):
        account = self.get_account(wallet, refresh=True)
        views = self._views_for_account(account)
        self._cache_views(wallet.address(), account, views)
        balances = [item for item in views if not item.asset.is_liquidity_pool]
        positions = [
            self._liquidity_position(item, refresh=True)
            for item in views
            if item.asset.is_liquidity_pool and item.balance > 0
        ]
        return balances, positions

    def has_cached_account(self, wallet) -> bool:
        """Return true only when a prior successful account load is cached."""
        return bool(
            self.datastore.get_balances(self.network_name, wallet.address())
        )

    def get_cached_portfolio_views(self, wallet):
        """Build a portfolio only from local cache; never touch Horizon."""
        raw_balances = self.datastore.get_balances(
            self.network_name,
            wallet.address(),
        )
        if not raw_balances:
            return [], []

        views = [_cached_balance_view(item) for item in raw_balances]
        views.sort(key=_balance_sort_key)
        balances = [item for item in views if not item.asset.is_liquidity_pool]
        positions = [
            self._liquidity_position(item, refresh=False)
            for item in views
            if item.asset.is_liquidity_pool and item.balance > 0
        ]
        return balances, positions

    def _views_for_account(self, account: dict):
        base_reserve = self.adapter.get_base_reserve_stroops()
        views = self.availability.balance_views(account, base_reserve)
        return sorted(views, key=_balance_sort_key)

    def _restore_cached_issuer_domains(self, address: str, balances: list[dict]) -> None:
        cached = {
            _raw_balance_key(item): item
            for item in self.datastore.get_balances(self.network_name, address)
        }
        for raw in balances:
            previous = cached.get(_raw_balance_key(raw))
            if previous is not None and ISSUER_DOMAIN_CACHE_KEY in previous:
                raw[ISSUER_DOMAIN_CACHE_KEY] = previous[ISSUER_DOMAIN_CACHE_KEY]

    def _enrich_issuer_domains(self, balances: list[dict]) -> None:
        domains = {
            str(raw.get("asset_issuer")): str(raw.get(ISSUER_DOMAIN_CACHE_KEY) or "")
            for raw in balances
            if raw.get("asset_issuer") and ISSUER_DOMAIN_CACHE_KEY in raw
        }
        attempted = set(domains)
        for raw in balances:
            issuer = raw.get("asset_issuer")
            if not issuer or raw.get("asset_type") in {"native", "liquidity_pool_shares"}:
                continue
            issuer = str(issuer)
            if ISSUER_DOMAIN_CACHE_KEY in raw:
                continue
            if issuer in domains:
                raw[ISSUER_DOMAIN_CACHE_KEY] = domains[issuer]
                continue
            if issuer in attempted:
                continue
            attempted.add(issuer)
            try:
                issuer_account = self.adapter.get_account(issuer)
            except FresnicaError:
                # A transient issuer lookup must not become a permanent negative cache.
                continue
            domain = str(issuer_account.get("home_domain") or "").strip()
            domains[issuer] = domain
            raw[ISSUER_DOMAIN_CACHE_KEY] = domain

    def _cache_views(self, address: str, account: dict, views: list[BalanceView]) -> None:
        """Persist presentation-safe availability alongside raw balance rows.

        Issued-asset availability can always be recomputed from a Horizon balance
        row. Native availability additionally depends on account reserve metadata,
        so the last computed value is retained for instant cached presentation.
        """
        by_key = {_view_key(view): view for view in views}
        cached = []
        for raw in account.get("balances", []):
            item = dict(raw)
            view = by_key.get(_raw_balance_key(raw))
            if view is not None and view.available is not None:
                item["_fresnica_available"] = str(view.available)
            cached.append(item)
        self.datastore.save_balances(self.network_name, address, cached)

    def _liquidity_position(
        self,
        balance_view,
        refresh: bool = True,
    ) -> LiquidityPositionView:
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
        if refresh:
            try:
                pool = self.adapter.get_liquidity_pool(pool_id)
                self.datastore.save_liquidity_pool(self.network_name, pool_id, pool)
            except (FresnicaError, ValueError, ArithmeticError) as exc:
                lookup_error = exc
                pool = self.datastore.get_liquidity_pool(self.network_name, pool_id)
        else:
            pool = self.datastore.get_liquidity_pool(self.network_name, pool_id)

        if pool is None:
            return LiquidityPositionView(
                pool_id=pool_id,
                shares=shares,
                error=(
                    str(lookup_error)
                    if lookup_error is not None
                    else "Liquidity pool details not cached yet"
                ),
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


def _cached_balance_view(raw: dict) -> BalanceView:
    asset_type = raw.get("asset_type")
    if asset_type == "native":
        asset = Asset("XLM")
    elif asset_type == "liquidity_pool_shares":
        asset = Asset("LP", liquidity_pool_id=raw.get("liquidity_pool_id"))
    else:
        asset = Asset(raw.get("asset_code", ""), raw.get("asset_issuer"))

    balance = Decimal(str(raw.get("balance", "0")))
    selling = Decimal(str(raw.get("selling_liabilities", "0")))
    buying = Decimal(str(raw.get("buying_liabilities", "0")))
    cached_available = raw.get("_fresnica_available")
    if cached_available is not None:
        available = Decimal(str(cached_available))
    elif asset.is_liquidity_pool:
        available = balance
    elif asset.is_native:
        # Never overstate spendable XLM when an older cache predates the stored
        # reserve-aware availability value. The background refresh fills it in.
        available = None
    else:
        available = max(balance - selling, Decimal("0"))

    return BalanceView(
        asset=asset,
        balance=balance,
        selling_liabilities=selling,
        buying_liabilities=buying,
        available=available,
        raw=raw,
    )


def _raw_balance_key(raw: dict):
    asset_type = raw.get("asset_type")
    if asset_type == "native":
        return ("native", "", "")
    if asset_type == "liquidity_pool_shares":
        return ("pool", raw.get("liquidity_pool_id") or "", "")
    return ("asset", raw.get("asset_code") or "", raw.get("asset_issuer") or "")


def _view_key(view: BalanceView):
    asset = view.asset
    if asset.is_native:
        return ("native", "", "")
    if asset.is_liquidity_pool:
        return ("pool", asset.liquidity_pool_id or "", "")
    return ("asset", asset.code, asset.issuer or "")


def _balance_sort_key(item):
    asset = item.asset
    if asset.is_native:
        return (0, "", "")
    if asset.is_liquidity_pool:
        return (2, asset.liquidity_pool_id or "", "")
    return (1, asset.code.upper(), asset.issuer or "")
