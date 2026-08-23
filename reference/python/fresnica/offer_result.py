"""Decode user-relevant SDEX state from Stellar transaction result XDR."""

from stellar_sdk import xdr as stellar_xdr

from .models import OfferSubmissionOutcome


def parse_offer_submission_outcome(result_xdr: str | None) -> OfferSubmissionOutcome | None:
    """Return optional ManageOffer metadata without making it transaction-critical."""
    if not result_xdr:
        return None
    try:
        payload = stellar_xdr.TransactionResult.from_xdr(result_xdr).to_json_dict()
    except Exception:
        # This is post-submit display metadata. A future/invalid XDR shape must
        # never rewrite a confirmed transaction success as a client failure.
        return None
    return _offer_outcome_from_result_json(payload)


def _offer_outcome_from_result_json(payload: dict) -> OfferSubmissionOutcome | None:
    result = payload.get("result")
    if not isinstance(result, dict):
        return None
    operations = result.get("txsuccess")
    if not isinstance(operations, list):
        return None

    # ChangeTrust may precede the offer operation. Walk backwards and select the
    # actual ManageBuyOffer / ManageSellOffer result rather than assuming index 0.
    for operation in reversed(operations):
        if not isinstance(operation, dict):
            continue
        inner = operation.get("opinner")
        if not isinstance(inner, dict):
            continue
        manage_result = inner.get("manage_buy_offer")
        if manage_result is None:
            manage_result = inner.get("manage_sell_offer")
        if not isinstance(manage_result, dict):
            continue
        success = manage_result.get("success")
        if not isinstance(success, dict):
            continue

        claimed = success.get("offers_claimed")
        claimed_count = len(claimed) if isinstance(claimed, list) else 0
        offer_result = success.get("offer")
        offer_id = None

        if offer_result == "deleted":
            effect = "deleted"
        elif isinstance(offer_result, dict) and "created" in offer_result:
            effect = "created"
            entry = offer_result.get("created")
            if isinstance(entry, dict) and entry.get("offer_id") is not None:
                offer_id = str(entry["offer_id"])
        elif isinstance(offer_result, dict) and "updated" in offer_result:
            effect = "updated"
            entry = offer_result.get("updated")
            if isinstance(entry, dict) and entry.get("offer_id") is not None:
                offer_id = str(entry["offer_id"])
        else:
            return None

        return OfferSubmissionOutcome(
            effect=effect,
            claimed_offer_count=claimed_count,
            offer_id=offer_id,
        )

    return None
