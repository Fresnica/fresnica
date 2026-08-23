"""User-oriented domain models.

Raw Horizon JSON remains available at the adapter/service boundary. These
models are only used where Fresnica needs to organize data for a human-facing
workflow.
"""

from dataclasses import dataclass, field
from decimal import Decimal
from typing import Literal

from .errors import InvalidAssetError


@dataclass(frozen=True)
class Asset:
    code: str
    issuer: str | None = None
    liquidity_pool_id: str | None = None

    @property
    def is_native(self) -> bool:
        return self.code.upper() == "XLM" and self.issuer is None and self.liquidity_pool_id is None

    @property
    def is_liquidity_pool(self) -> bool:
        return self.liquidity_pool_id is not None

    @property
    def display(self) -> str:
        if self.is_native:
            return "XLM"
        if self.is_liquidity_pool:
            pool_id = self.liquidity_pool_id or ""
            return f"LP:{pool_id[:8]}" if pool_id else "LP"
        return self.code

    @classmethod
    def parse(cls, value: str) -> "Asset":
        text = value.strip()
        if text.upper() == "XLM":
            return cls("XLM")

        if ":" not in text:
            raise InvalidAssetError(
                "Issued assets must use CODE:ISSUER, for example USDC:G..."
            )

        code, issuer = text.split(":", 1)
        code = code.strip()
        issuer = issuer.strip()
        if not code or not issuer:
            raise InvalidAssetError("Invalid asset. Expected CODE:ISSUER")
        return cls(code, issuer)

    @classmethod
    def from_horizon_string(cls, value: str) -> "Asset":
        text = value.strip()
        if text.lower() == "native" or text.upper() == "XLM":
            return cls("XLM")
        return cls.parse(text)


OrderSide = Literal["buy", "sell"]


@dataclass(frozen=True)
class MarketPair:
    """A user-facing market orientation: base / counter."""

    base: Asset
    counter: Asset


@dataclass(frozen=True)
class PriceRatio:
    """Exact Stellar price fraction."""

    n: int
    d: int

    def __post_init__(self) -> None:
        if self.n <= 0 or self.d <= 0:
            raise ValueError("Price ratio numerator and denominator must be positive")


@dataclass
class OpenOffer:
    """Canonical current offer state as stored on the Stellar ledger."""

    offer_id: str
    selling: Asset
    buying: Asset
    selling_amount: Decimal
    price_r: PriceRatio
    seller: str | None = None
    last_modified_ledger: int | None = None
    last_modified_time: str | None = None
    raw: dict = field(default_factory=dict)


@dataclass(frozen=True)
class OfferView:
    """An open offer projected into a selected market orientation."""

    pair: MarketPair
    side: OrderSide
    amount: Decimal
    price: Decimal
    total: Decimal


@dataclass(frozen=True)
class OfferIntent:
    """User intent: amount is always base units and price is counter/base."""

    pair: MarketPair
    side: OrderSide
    amount: Decimal
    price: Decimal


@dataclass
class AccountTrade:
    """One Horizon trade involving the wallet, before user-facing aggregation."""

    trade_id: str
    pair: MarketPair
    base_amount: Decimal
    counter_amount: Decimal
    price_r: PriceRatio
    side: OrderSide
    time: str | None
    paging_token: str | None = None
    base_account: str | None = None
    counter_account: str | None = None
    base_offer_id: str | None = None
    counter_offer_id: str | None = None
    transaction_hash: str | None = None
    raw: dict = field(default_factory=dict)


@dataclass
class AccountTradeSegment:
    """Consecutive fills from one wallet offer at one exact price."""

    segment_key: str
    pair: MarketPair
    side: OrderSide
    base_amount: Decimal
    counter_amount: Decimal
    price_r: PriceRatio
    user_offer_id: str | None
    trade_count: int
    first_time: str | None
    last_time: str | None
    first_trade_id: str
    last_trade_id: str
    raw: list[dict] = field(default_factory=list)


@dataclass
class BalanceView:
    asset: Asset
    balance: Decimal
    selling_liabilities: Decimal = Decimal("0")
    buying_liabilities: Decimal = Decimal("0")
    available: Decimal | None = None
    raw: dict = field(default_factory=dict)


@dataclass(frozen=True)
class LiquidityReserveView:
    asset: Asset
    amount: Decimal


@dataclass
class LiquidityPositionView:
    pool_id: str
    shares: Decimal
    reserves: list[LiquidityReserveView] = field(default_factory=list)
    raw: dict = field(default_factory=dict)
    error: str | None = None


@dataclass
class TransactionResult:
    hash: str
    ledger: int | None
    successful: bool
    raw: dict = field(default_factory=dict)


@dataclass
class OperationView:
    operation_type: str
    created_at: str | None
    summary: str
    raw: dict = field(default_factory=dict)


@dataclass
class ActivityView:
    """One user-facing activity, usually one Stellar transaction."""

    operation_type: str
    created_at: str | None
    summary: str
    transaction_hash: str | None = None
    operation_count: int = 1
    operations: list[OperationView] = field(default_factory=list)
    raw: list[dict] = field(default_factory=list)
