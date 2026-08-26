"""Project domain review models into one user-facing presentation shape."""

from dataclasses import dataclass
from typing import Literal

from .review import OfferReview, TransactionReview, TrustlineReview


ReviewKind = Literal["transfer", "offer", "trustline"]


@dataclass(frozen=True)
class ReviewField:
    label: str
    value: str


@dataclass(frozen=True)
class ReviewPresentation:
    """UI-neutral transaction review consumed by CLI, TUI, and future clients."""

    kind: ReviewKind
    title: str
    summary: str
    fields: tuple[ReviewField, ...]
    warnings: tuple[str, ...] = ()


def project_review(
    review: TransactionReview | OfferReview | TrustlineReview,
) -> ReviewPresentation:
    if isinstance(review, TransactionReview):
        return _project_transfer(review)
    if isinstance(review, OfferReview):
        return _project_offer(review)
    if isinstance(review, TrustlineReview):
        return _project_trustline(review)
    raise TypeError(f"Unsupported review model: {type(review).__name__}")


def review_text(review: TransactionReview | OfferReview | TrustlineReview) -> str:
    """Plain text rendering with semantics shared by every interactive surface."""
    presentation = project_review(review)
    lines = [presentation.summary, ""]
    lines.extend(f"{field.label}: {field.value}" for field in presentation.fields)
    if presentation.warnings:
        lines.append("")
        lines.extend(f"Warning: {warning}" for warning in presentation.warnings)
    return "\n".join(lines)


def _project_transfer(review: TransactionReview) -> ReviewPresentation:
    create_account = review.operation == "create_account"
    title = "Confirm account creation" if create_account else "Confirm transfer"
    summary = (
        f"Create and fund a Stellar account with {review.amount} XLM"
        if create_account
        else f"Send {review.amount} {review.asset}"
    )
    destination = (
        f"{review.contact_name} ({review.destination})"
        if review.contact_name
        else review.destination
    )
    fields = [
        ReviewField("From", f"{review.wallet_name} ({review.source})"),
        ReviewField("To", destination),
        ReviewField("Starting balance" if create_account else "Amount", f"{review.amount} {review.asset}"),
    ]
    if review.memo:
        memo_label = f"Memo ({review.memo_type})" if review.memo_type and review.memo_type != "text" else "Memo"
        fields.append(ReviewField(memo_label, review.memo))
    fields.extend(
        [
            ReviewField("Fee", f"{review.fee} XLM"),
            ReviewField("Network", review.network),
        ]
    )
    return ReviewPresentation(
        kind="transfer",
        title=title,
        summary=summary,
        fields=tuple(fields),
    )


def _project_offer(review: OfferReview) -> ReviewPresentation:
    warnings = (
        (
            f"Creates trustline for {review.trustline_asset}"
            + (f" with limit {review.trustline_limit}" if review.trustline_limit else ""),
        )
        if review.trustline_asset
        else ()
    )

    if review.action == "cancel":
        side = review.side.upper() if review.side else None
        summary = f"Cancel {side} limit offer" if side else "Cancel Stellar offer"
        fields = [ReviewField("Offer", f"#{review.offer_id}")]
        if review.side:
            fields.append(ReviewField("Pair", f"{review.base_asset} / {review.counter_asset}"))
            if review.amount is not None:
                fields.append(ReviewField("Remaining", f"{review.amount} {review.base_asset}"))
            if review.price is not None:
                fields.append(
                    ReviewField(
                        "Limit price",
                        f"{review.price} {review.counter_asset}/{review.base_asset}",
                    )
                )
            if review.total is not None:
                fields.append(
                    ReviewField(
                        "Max spend" if review.side == "buy" else "Min receive",
                        f"{review.total} {review.counter_asset}",
                    )
                )
        else:
            fields.extend(
                [
                    ReviewField("Selling", review.base_asset),
                    ReviewField("Buying", review.counter_asset),
                ]
            )
        fields.extend(
            [
                ReviewField("Fee", f"{review.fee} XLM"),
                ReviewField("Network", review.network),
            ]
        )
        return ReviewPresentation(
            kind="offer",
            title="Confirm offer cancellation",
            summary=summary,
            fields=tuple(fields),
            warnings=warnings,
        )

    side = (review.side or "").upper()
    verb = "Create" if review.action == "create" else "Update"
    fields = [
        ReviewField("Pair", f"{review.base_asset} / {review.counter_asset}"),
        ReviewField("Amount", f"{review.amount} {review.base_asset}"),
        ReviewField(
            "Limit price",
            f"{review.price} {review.counter_asset}/{review.base_asset}",
        ),
        ReviewField(
            "Max spend" if review.side == "buy" else "Min receive",
            f"{review.total} {review.counter_asset}",
        ),
    ]
    if review.offer_id:
        fields.append(ReviewField("Offer", f"#{review.offer_id}"))
    fields.extend(
        [
            ReviewField("Fee", f"{review.fee} XLM"),
            ReviewField("Network", review.network),
        ]
    )
    return ReviewPresentation(
        kind="offer",
        title="Confirm offer",
        summary=f"{verb} {side} limit offer",
        fields=tuple(fields),
        warnings=warnings,
    )


def _project_trustline(review: TrustlineReview) -> ReviewPresentation:
    if review.action == "add":
        summary = f"Add trustline for {review.asset}"
        title = "Confirm trustline creation"
    elif review.action == "limit":
        summary = f"Change trustline limit for {review.asset}"
        title = "Confirm trustline limit"
    elif review.action == "remove":
        summary = f"Remove trustline for {review.asset}"
        title = "Confirm trustline removal"
    else:
        raise ValueError(f"Unsupported trustline action: {review.action}")

    fields = [
        ReviewField("Wallet", f"{review.wallet_name} ({review.source})"),
        ReviewField("Asset", review.asset),
    ]
    if review.limit is not None:
        fields.append(ReviewField("Limit", review.limit))
    fields.extend(
        [
            ReviewField("Fee", f"{review.fee} XLM"),
            ReviewField("Network", review.network),
        ]
    )
    warnings = (
        ("Removing a trustline is irreversible unless you create it again.",)
        if review.action == "remove"
        else ()
    )
    return ReviewPresentation(
        kind="trustline",
        title=title,
        summary=summary,
        fields=tuple(fields),
        warnings=warnings,
    )
