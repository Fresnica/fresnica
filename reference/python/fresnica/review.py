"""Human-readable transaction review models."""

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
    operation: str = "payment"
    memo: str | None = None
    contact_name: str | None = None


@dataclass(frozen=True)
class OfferReview:
    wallet_name: str
    source: str
    action: str
    side: str | None
    base_asset: str
    counter_asset: str
    amount: str | None
    price: str | None
    total: str | None
    fee: str
    network: str
    offer_id: str | None = None
    trustline_asset: str | None = None


@dataclass(frozen=True)
class TrustlineReview:
    wallet_name: str
    source: str
    action: str
    asset: str
    limit: str | None
    fee: str
    network: str
