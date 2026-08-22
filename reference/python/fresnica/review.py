"""Human readable transaction review model."""

from dataclasses import dataclass


@dataclass
class TransactionReview:
    source: str
    destination: str
    asset: str
    amount: str
    fee: str | None = None
    network: str | None = None
