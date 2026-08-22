"""User-oriented domain models.

Raw Horizon JSON remains available at the adapter/service boundary. These
models are only used where Fresnica needs to organize data for a human-facing
workflow.
"""

from dataclasses import dataclass, field
from decimal import Decimal

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
