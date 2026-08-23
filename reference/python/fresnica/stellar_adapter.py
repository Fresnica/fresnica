"""Thin boundary around Stellar Python SDK network operations."""

from stellar_sdk import Asset as StellarAsset
from stellar_sdk import Price, Server, TransactionBuilder
from stellar_sdk.exceptions import NotFoundError, SdkError

from .errors import NetworkError, TransactionError
from .models import Asset, PriceRatio
from .network import Network


class StellarAdapter:
    def __init__(self, network: Network):
        self.network = network
        self.server = Server(network.horizon_url)
        self._base_reserve_stroops: int | None = None

    def get_account(self, address: str) -> dict:
        try:
            return self.server.accounts().account_id(address).call()
        except SdkError as exc:
            raise NetworkError(
                f"Unable to load Stellar account {address}",
                details=_sdk_error_details(exc),
            ) from exc

    def account_exists(self, address: str) -> bool:
        try:
            self.server.load_account(address)
            return True
        except NotFoundError:
            return False
        except SdkError as exc:
            raise NetworkError(
                f"Unable to check Stellar account {address}",
                details=_sdk_error_details(exc),
            ) from exc

    def get_balances(self, address: str) -> list[dict]:
        return self.get_account(address).get("balances", [])

    def get_operations(
        self,
        address: str,
        limit: int = 20,
        cursor: str | None = None,
        desc: bool = True,
    ) -> dict:
        try:
            builder = (
                self.server.operations()
                .for_account(address)
                .order(desc=desc)
                .limit(limit)
            )
            if cursor is not None:
                builder = builder.cursor(cursor)
            return builder.call()
        except SdkError as exc:
            raise NetworkError(
                f"Unable to load operations for {address}",
                details=_sdk_error_details(exc),
            ) from exc

    def get_liquidity_pool(self, liquidity_pool_id: str) -> dict:
        try:
            return (
                self.server.liquidity_pools()
                .liquidity_pool(liquidity_pool_id)
                .call()
            )
        except SdkError as exc:
            raise NetworkError(
                f"Unable to load liquidity pool {liquidity_pool_id}",
                details=_sdk_error_details(exc),
            ) from exc

    def get_orderbook(self, selling: Asset, buying: Asset) -> dict:
        try:
            return self.server.orderbook(
                self.to_sdk_asset(selling),
                self.to_sdk_asset(buying),
            ).call()
        except SdkError as exc:
            raise NetworkError(
                "Unable to load Stellar order book",
                details=_sdk_error_details(exc),
            ) from exc

    def get_offers(self, address: str, limit: int = 20) -> dict:
        try:
            return (
                self.server.offers()
                .for_account(address)
                .order(desc=True)
                .limit(limit)
                .call()
            )
        except SdkError as exc:
            raise NetworkError(
                f"Unable to load offers for {address}",
                details=_sdk_error_details(exc),
            ) from exc

    def get_trades(self, base: Asset, counter: Asset, limit: int = 20) -> dict:
        try:
            return (
                self.server.trades()
                .for_asset_pair(
                    self.to_sdk_asset(base),
                    self.to_sdk_asset(counter),
                )
                .order(desc=True)
                .limit(limit)
                .call()
            )
        except SdkError as exc:
            raise NetworkError(
                "Unable to load Stellar trades",
                details=_sdk_error_details(exc),
            ) from exc

    def get_account_trades(
        self,
        address: str,
        limit: int = 200,
        cursor: str | None = None,
        desc: bool = True,
    ) -> dict:
        try:
            builder = (
                self.server.trades()
                .for_account(address)
                .order(desc=desc)
                .limit(limit)
            )
            if cursor is not None:
                builder = builder.cursor(cursor)
            return builder.call()
        except SdkError as exc:
            raise NetworkError(
                f"Unable to load trades for {address}",
                details=_sdk_error_details(exc),
            ) from exc

    def get_trade_aggregations(
        self,
        base: Asset,
        counter: Asset,
        resolution: int,
        start_time: int | None = None,
        end_time: int | None = None,
        offset: int | None = None,
        limit: int = 100,
    ) -> dict:
        try:
            return (
                self.server.trade_aggregations(
                    self.to_sdk_asset(base),
                    self.to_sdk_asset(counter),
                    resolution,
                    start_time=start_time,
                    end_time=end_time,
                    offset=offset,
                )
                .order(desc=True)
                .limit(limit)
                .call()
            )
        except SdkError as exc:
            raise NetworkError(
                "Unable to load Stellar trade aggregations",
                details=_sdk_error_details(exc),
            ) from exc

    def fetch_base_fee(self) -> int:
        try:
            return int(self.server.fetch_base_fee())
        except SdkError as exc:
            raise NetworkError(
                "Unable to fetch Stellar base fee",
                details=_sdk_error_details(exc),
            ) from exc

    def get_latest_ledger(self) -> dict:
        try:
            response = self.server.ledgers().order(desc=True).limit(1).call()
            records = response.get("_embedded", {}).get("records", [])
            if not records:
                raise NetworkError("Horizon returned no ledger records")
            return records[0]
        except SdkError as exc:
            raise NetworkError(
                "Unable to fetch latest Stellar ledger",
                details=_sdk_error_details(exc),
            ) from exc

    def get_base_reserve_stroops(self) -> int:
        if self._base_reserve_stroops is None:
            self._base_reserve_stroops = int(
                self.get_latest_ledger()["base_reserve_in_stroops"]
            )
        return self._base_reserve_stroops

    def build_payment(
        self,
        source: str,
        destination: str,
        asset: Asset,
        amount: str,
        base_fee: int,
        memo: str | None = None,
        timeout: int = 30,
        create_destination: bool = False,
    ):
        try:
            source_account = self.server.load_account(source)
            builder = TransactionBuilder(
                source_account=source_account,
                network_passphrase=self.network.passphrase,
                base_fee=base_fee,
            )
            if create_destination:
                builder = builder.append_create_account_op(
                    destination=destination,
                    starting_balance=amount,
                )
            else:
                builder = builder.append_payment_op(
                    destination=destination,
                    asset=self.to_sdk_asset(asset),
                    amount=amount,
                )
            if memo:
                builder = builder.add_text_memo(memo)
            return builder.set_timeout(timeout).build()
        except SdkError as exc:
            raise TransactionError(
                "Unable to build Stellar payment transaction",
                details=_sdk_error_details(exc),
            ) from exc

    def build_manage_sell_offer(
        self,
        source: str,
        selling: Asset,
        buying: Asset,
        amount: str,
        price,
        base_fee: int,
        offer_id: int = 0,
        trustline_asset: Asset | None = None,
        timeout: int = 30,
    ):
        return self._build_offer_transaction(
            source=source,
            selling=selling,
            buying=buying,
            amount=amount,
            price=price,
            base_fee=base_fee,
            offer_id=offer_id,
            buy=False,
            trustline_asset=trustline_asset,
            timeout=timeout,
        )

    def build_manage_buy_offer(
        self,
        source: str,
        selling: Asset,
        buying: Asset,
        buy_amount: str,
        price,
        base_fee: int,
        offer_id: int = 0,
        trustline_asset: Asset | None = None,
        timeout: int = 30,
    ):
        return self._build_offer_transaction(
            source=source,
            selling=selling,
            buying=buying,
            amount=buy_amount,
            price=price,
            base_fee=base_fee,
            offer_id=offer_id,
            buy=True,
            trustline_asset=trustline_asset,
            timeout=timeout,
        )

    def _build_offer_transaction(
        self,
        source: str,
        selling: Asset,
        buying: Asset,
        amount: str,
        price,
        base_fee: int,
        offer_id: int,
        buy: bool,
        trustline_asset: Asset | None,
        timeout: int,
    ):
        try:
            source_account = self.server.load_account(source)
            builder = TransactionBuilder(
                source_account=source_account,
                network_passphrase=self.network.passphrase,
                base_fee=base_fee,
            )
            if trustline_asset is not None:
                builder = builder.append_change_trust_op(
                    asset=self.to_sdk_asset(trustline_asset)
                )
            kwargs = {
                "selling": self.to_sdk_asset(selling),
                "buying": self.to_sdk_asset(buying),
                "amount": amount,
                "price": self.to_sdk_price(price),
                "offer_id": offer_id,
            }
            if buy:
                builder = builder.append_manage_buy_offer_op(**kwargs)
            else:
                builder = builder.append_manage_sell_offer_op(**kwargs)
            return builder.set_timeout(timeout).build()
        except SdkError as exc:
            raise TransactionError(
                "Unable to build Stellar offer transaction",
                details=_sdk_error_details(exc),
            ) from exc

    def submit_transaction(self, transaction) -> dict:
        try:
            return self.server.submit_transaction(transaction)
        except SdkError as exc:
            raise TransactionError(
                "Stellar transaction submission failed",
                details=_sdk_error_details(exc),
            ) from exc

    @staticmethod
    def to_sdk_asset(asset: Asset):
        if asset.is_native:
            return StellarAsset.native()
        if asset.is_liquidity_pool:
            raise ValueError("Liquidity pool shares are not payment assets")
        return StellarAsset(asset.code, asset.issuer)

    @staticmethod
    def to_sdk_price(price):
        if isinstance(price, PriceRatio):
            return Price(price.n, price.d)
        return price


def _sdk_error_details(exc: SdkError) -> str:
    parts = [type(exc).__name__]
    status = getattr(exc, "status", None)
    title = getattr(exc, "title", None)
    detail = getattr(exc, "detail", None)
    extras = getattr(exc, "extras", None)

    if status is not None:
        parts.append(f"status={status}")
    if title:
        parts.append(str(title))
    if detail:
        parts.append(str(detail))
    if isinstance(extras, dict) and extras.get("result_codes") is not None:
        parts.append(f"result_codes={extras['result_codes']}")
    return "; ".join(parts)
