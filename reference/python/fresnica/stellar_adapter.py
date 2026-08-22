"""Thin boundary around Stellar Python SDK network operations."""

from stellar_sdk import Asset as StellarAsset
from stellar_sdk import Server, TransactionBuilder
from stellar_sdk.exceptions import SdkError

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
            raise NetworkError(f"Unable to load Stellar account {address}") from exc

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
            raise NetworkError(f"Unable to load operations for {address}") from exc

    def fetch_base_fee(self) -> int:
        try:
            return int(self.server.fetch_base_fee())
        except SdkError as exc:
            raise NetworkError("Unable to fetch Stellar base fee") from exc

    def get_latest_ledger(self) -> dict:
        try:
            response = self.server.ledgers().order(desc=True).limit(1).call()
            records = response.get("_embedded", {}).get("records", [])
            if not records:
                raise NetworkError("Horizon returned no ledger records")
            return records[0]
        except SdkError as exc:
            raise NetworkError("Unable to fetch latest Stellar ledger") from exc

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
    ):
        try:
            source_account = self.server.load_account(source)
            builder = TransactionBuilder(
                source_account=source_account,
                network_passphrase=self.network.passphrase,
                base_fee=base_fee,
            ).append_payment_op(
                destination=destination,
                asset=self.to_sdk_asset(asset),
                amount=amount,
            )
            if memo:
                builder = builder.add_text_memo(memo)
            return builder.set_timeout(timeout).build()
        except SdkError as exc:
            raise TransactionError("Unable to build Stellar payment transaction") from exc

    def submit_transaction(self, transaction) -> dict:
        try:
            return self.server.submit_transaction(transaction)
        except SdkError as exc:
            raise TransactionError("Stellar transaction submission failed") from exc

    @staticmethod
    def to_sdk_asset(asset: Asset):
        if asset.is_native:
            return StellarAsset.native()
        return StellarAsset(asset.code, asset.issuer)
