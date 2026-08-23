from stellar_sdk import Keypair

from fresnica.models import ActivityView, OperationView
from fresnica.tui.activity_presentation import activity_text


def _activity(account, raw, summary="activity"):
    operation = OperationView(
        operation_type=raw["type"],
        created_at="2026-08-23T00:00:00Z",
        summary=summary,
        raw=raw,
    )
    return ActivityView(
        operation_type=operation.operation_type,
        created_at=operation.created_at,
        summary=summary,
        transaction_hash="tx",
        operation_count=1,
        operations=[operation],
        raw=[raw],
    )


def test_suspicious_claimable_is_visible_with_warning_and_dimmed_text():
    account = Keypair.random().public_key
    activity = _activity(
        account,
        {
            "type": "create_claimable_balance",
            "_fresnica_unsolicited_claimable": True,
        },
        "Incoming claimable asset",
    )

    rendered = activity_text(activity, activity.summary, account)

    assert rendered.plain == "⚠ Incoming claimable asset"
    assert any(span.style == "dim" for span in rendered.spans)


def test_payment_markers_preserve_direction_without_relying_on_color():
    account = Keypair.random().public_key
    other = Keypair.random().public_key
    incoming = _activity(
        account,
        {"type": "payment", "from": other, "to": account},
        "Received 1 XLM",
    )
    outgoing = _activity(
        account,
        {"type": "payment", "from": account, "to": other},
        "Sent 1 XLM",
    )

    assert activity_text(incoming, incoming.summary, account).plain.startswith("↓ ")
    assert activity_text(outgoing, outgoing.summary, account).plain.startswith("↑ ")
