"""Human-readable transaction review model."""

from dataclasses import dataclass


@dataclass(frozen=True)
class TransactionReview:
    wallet_name: str
    source: str
    destination: str
    asset: str
    amount: str
    fee: str
    network: str
    memo: str | None = None
    contact_name: str | None = None
