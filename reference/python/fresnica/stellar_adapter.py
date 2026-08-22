"""Thin boundary around Stellar Python SDK network operations."""

from stellar_sdk import Asset as StellarAsset
from stellar_sdk import Server, TransactionBuilder
from stellar_sdk.exceptions import NotFoundError, SdkError

from .errors import NetworkError, TransactionError
from .models import Asset
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

    def get_operations(self, address: str, limit: int = 20) -> dict:
        try:
            return (
                self.server.operations()
                .for_account(address)
                .order(desc=True)
                .limit(limit)
                .call()
            )
        except SdkError as exc:
            raise NetworkError(
                f"Unable to load operations for {address}",
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
        return StellarAsset(asset.code, asset.issuer)


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
