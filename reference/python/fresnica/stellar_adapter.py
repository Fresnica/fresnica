"""Thin boundary around Stellar Python SDK network operations."""

from stellar_sdk import Asset as StellarAsset
from stellar_sdk import Price, Server, TransactionBuilder
from stellar_sdk.exceptions import NotFoundError, SdkError
from stellar_sdk.sep.exceptions import AccountRequiresMemoError

from .errors import MemoRequiredError, NetworkError, TransactionError
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
            ).limit(20).call()
        except SdkError as exc:
            raise NetworkError(
                "Unable to load Stellar order book",
                details=_sdk_error_details(exc),
            ) from exc

    def stream_orderbook(self, selling: Asset, buying: Asset):
        """Yield Horizon order-book snapshots through the Stellar SDK SSE client."""
        try:
            yield from self.server.orderbook(
                self.to_sdk_asset(selling),
                self.to_sdk_asset(buying),
            ).limit(20).stream()
        except SdkError as exc:
            raise NetworkError(
                "Stellar order-book realtime stream disconnected",
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

    def get_offer(self, offer_id: str | int) -> dict:
        try:
            return self.server.offers().offer(offer_id).call()
        except SdkError as exc:
            raise NetworkError(
                f"Unable to load Stellar offer {offer_id}",
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

    def stream_trades(
        self,
        base: Asset,
        counter: Asset,
        cursor: str | None = None,
    ):
        """Yield pair trades through the Stellar SDK SSE client."""
        try:
            builder = self.server.trades().for_asset_pair(
                self.to_sdk_asset(base),
                self.to_sdk_asset(counter),
            )
            if cursor is not None:
                builder = builder.cursor(cursor)
            yield from builder.stream()
        except SdkError as exc:
            raise NetworkError(
                "Stellar trade realtime stream disconnected",
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

    def build_change_trust(
        self,
        source: str,
        asset: Asset,
        base_fee: int,
        limit: str | None = None,
        timeout: int = 30,
    ):
        try:
            source_account = self.server.load_account(source)
            builder = TransactionBuilder(
                source_account=source_account,
                network_passphrase=self.network.passphrase,
                base_fee=base_fee,
            )
            kwargs = {"asset": self.to_sdk_asset(asset)}
            if limit is not None:
                kwargs["limit"] = limit
            builder = builder.append_change_trust_op(**kwargs)
            return builder.set_timeout(timeout).build()
        except SdkError as exc:
            raise TransactionError(
                "Unable to build Stellar trustline transaction",
                details=_sdk_error_details(exc),
            ) from exc

    def build_manage_sell_offer(
        self,
        source: str,
        selling: Asset,
        buying: Asset,
        amount: str,
        price,
        offer_id: int = 0,
        base_fee: int = 100,
        timeout: int = 30,
        trustline_asset: Asset | None = None,
    ):
        return self._build_manage_offer(
            source=source,
            selling=selling,
            buying=buying,
            amount=amount,
            price=price,
            offer_id=offer_id,
            base_fee=base_fee,
            timeout=timeout,
            trustline_asset=trustline_asset,
            buy=False,
        )

    def build_manage_buy_offer(
        self,
        source: str,
        selling: Asset,
        buying: Asset,
        amount: str,
        price,
        offer_id: int = 0,
        base_fee: int = 100,
        timeout: int = 30,
        trustline_asset: Asset | None = None,
    ):
        return self._build_manage_offer(
            source=source,
            selling=selling,
            buying=buying,
            amount=amount,
            price=price,
            offer_id=offer_id,
            base_fee=base_fee,
            timeout=timeout,
            trustline_asset=trustline_asset,
            buy=True,
        )

    def _build_manage_offer(
        self,
        source: str,
        selling: Asset,
        buying: Asset,
        amount: str,
        price,
        offer_id: int,
        base_fee: int,
        timeout: int,
        trustline_asset: Asset | None,
        buy: bool,
    ):
        try:
            source_account = self.server.load_account(source)
            builder = TransactionBuilder(
                source_account=source_account,
                network_passphrase=self.network.passphrase,
                base_fee=base_fee,
            )
            if trustline_asset is not None:
                builder = builder.append_change_trust_op(asset=self.to_sdk_asset(trustline_asset))
            if buy:
                builder = builder.append_manage_buy_offer_op(
                    selling=self.to_sdk_asset(selling),
                    buying=self.to_sdk_asset(buying),
                    amount=amount,
                    price=self.to_sdk_price(price),
                    offer_id=offer_id,
                )
            else:
                builder = builder.append_manage_sell_offer_op(
                    selling=self.to_sdk_asset(selling),
                    buying=self.to_sdk_asset(buying),
                    amount=amount,
                    price=self.to_sdk_price(price),
                    offer_id=offer_id,
                )
            return builder.set_timeout(timeout).build()
        except SdkError as exc:
            raise TransactionError(
                "Unable to build Stellar offer transaction",
                details=_sdk_error_details(exc),
            ) from exc

    @staticmethod
    def to_sdk_price(value):
        if isinstance(value, PriceRatio):
            return Price(value.n, value.d)
        return value

    @staticmethod
    def to_sdk_asset(asset: Asset):
        if asset.is_native:
            return StellarAsset.native()
        if asset.is_liquidity_pool:
            raise ValueError("Liquidity pool shares cannot be used as a direct Stellar asset")
        return StellarAsset(asset.code, asset.issuer)

    def submit_transaction(self, envelope) -> dict:
        try:
            return self.server.submit_transaction(envelope)
        except AccountRequiresMemoError as exc:
            raise MemoRequiredError(str(exc.account_id)) from exc
        except SdkError as exc:
            raise TransactionError(
                "Stellar transaction submission failed",
                details=_sdk_error_details(exc),
            ) from exc

    def get_transaction(self, tx_hash: str) -> dict:
        try:
            return self.server.transactions().transaction(tx_hash).call()
        except NotFoundError:
            raise
        except SdkError as exc:
            raise NetworkError(
                f"Unable to load Stellar transaction {tx_hash}",
                details=_sdk_error_details(exc),
            ) from exc


def _sdk_error_details(exc: Exception) -> str:
    response = getattr(exc, "response", None)
    if response is not None:
        payload = getattr(response, "json", None)
        if callable(payload):
            try:
                body = payload()
            except Exception:
                body = None
            if body:
                return str(body)
        text = getattr(response, "text", None)
        if text:
            return str(text)
    return str(exc)
