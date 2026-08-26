from fresnica.review import OfferReview, TransactionReview
from fresnica.review_presentation import project_review, review_text


def _fields(presentation):
    return {field.label: field.value for field in presentation.fields}


def test_transfer_review_does_not_expose_protocol_operation_name():
    review = TransactionReview(
        wallet_name="main",
        source="GSOURCE",
        destination="GDESTINATION",
        asset="USDC:GISSUER",
        amount="12.5",
        fee="0.00001",
        network="mainnet",
        operation="payment",
        memo="invoice-42",
    )

    presentation = project_review(review)
    fields = _fields(presentation)

    assert presentation.kind == "transfer"
    assert presentation.summary == "Send 12.5 USDC:GISSUER"
    assert fields["Amount"] == "12.5 USDC:GISSUER"
    assert fields["Memo"] == "invoice-42"
    assert "Operation" not in fields
    assert "payment" not in review_text(review).lower()


def test_create_account_review_uses_user_action_language():
    review = TransactionReview(
        wallet_name="main",
        source="GSOURCE",
        destination="GNEW",
        asset="XLM",
        amount="2",
        fee="0.00001",
        network="testnet",
        operation="create_account",
    )

    presentation = project_review(review)
    fields = _fields(presentation)

    assert presentation.title == "Confirm account creation"
    assert presentation.summary == "Create and fund a Stellar account with 2 XLM"
    assert fields["Starting balance"] == "2 XLM"
    assert "CreateAccount" not in review_text(review)


def test_buy_review_labels_total_as_max_spend_and_trustline_as_warning():
    review = OfferReview(
        wallet_name="main",
        source="GSOURCE",
        action="create",
        side="buy",
        base_asset="XRP:GXRP",
        counter_asset="USDC:GUSDC",
        amount="100",
        price="0.325",
        total="32.5",
        fee="0.00002",
        network="mainnet",
        trustline_asset="XRP:GXRP",
        trustline_limit="708269837873.6765",
    )

    presentation = project_review(review)
    fields = _fields(presentation)

    assert presentation.summary == "Create BUY limit offer"
    assert fields["Pair"] == "XRP:GXRP / USDC:GUSDC"
    assert fields["Max spend"] == "32.5 USDC:GUSDC"
    assert "Min receive" not in fields
    assert presentation.warnings == ("Creates trustline for XRP:GXRP with limit 708269837873.6765",)


def test_sell_review_labels_total_as_min_receive():
    review = OfferReview(
        wallet_name="main",
        source="GSOURCE",
        action="update",
        side="sell",
        base_asset="XRP:GXRP",
        counter_asset="USDC:GUSDC",
        amount="100",
        price="0.325",
        total="32.5",
        fee="0.00001",
        network="mainnet",
        offer_id="77",
    )

    fields = _fields(project_review(review))

    assert fields["Min receive"] == "32.5 USDC:GUSDC"
    assert fields["Offer"] == "#77"
    assert "Max spend" not in fields


def test_cancel_without_pair_context_is_explicitly_canonical():
    review = OfferReview(
        wallet_name="main",
        source="GSOURCE",
        action="cancel",
        side=None,
        base_asset="USDC:GUSDC",
        counter_asset="XRP:GXRP",
        amount=None,
        price=None,
        total=None,
        fee="0.00001",
        network="mainnet",
        offer_id="77",
    )

    presentation = project_review(review)
    fields = _fields(presentation)

    assert presentation.summary == "Cancel Stellar offer"
    assert fields["Selling"] == "USDC:GUSDC"
    assert fields["Buying"] == "XRP:GXRP"
    assert "Pair" not in fields


def test_cancel_with_pair_context_keeps_user_buy_orientation():
    review = OfferReview(
        wallet_name="main",
        source="GSOURCE",
        action="cancel",
        side="buy",
        base_asset="XRP:GXRP",
        counter_asset="USDC:GUSDC",
        amount="10",
        price="0.325",
        total="3.25",
        fee="0.00001",
        network="mainnet",
        offer_id="77",
    )

    presentation = project_review(review)
    fields = _fields(presentation)

    assert presentation.summary == "Cancel BUY limit offer"
    assert fields["Pair"] == "XRP:GXRP / USDC:GUSDC"
    assert fields["Remaining"] == "10 XRP:GXRP"
    assert fields["Max spend"] == "3.25 USDC:GUSDC"
