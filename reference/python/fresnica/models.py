"""Domain models used by Fresnica services.

Raw Stellar SDK/Horizon responses are preserved separately.
These models represent user-oriented concepts when needed.
"""

from dataclasses import dataclass
from decimal import Decimal


@dataclass
class Asset:
    code: str
    issuer: str | None = None


@dataclass
class BalanceView:
    asset: Asset
    balance: Decimal
    available: Decimal | None = None
