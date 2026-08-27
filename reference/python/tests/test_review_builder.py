from decimal import Decimal

from stellar_sdk import Keypair

from fresnica.models import Asset, MarketPair, OpenOffer, PriceRatio
from fresnica.network import MAINNET
from fresnica.offer_service import OfferService
from fresnica.submit_service import SubmitService
from fresnica.transaction_builder_service import TransactionBuilderService
from fresnica.transaction_service import TransactionService
from fresnica.trustline_policy import FRESNICA_TRUSTLINE_LIMIT_TEXT
from fresnica.wallet import Wallet


class FakeEnvelope:
    def sign(self, keypair):
        pass


class FakeAdapter:
    network = MAINNET

    def fetch_base_fee(self):
        return 100

    def get_account(self, address):
        return {
            "account_id": address,
            "subentry_count": 0,
            "num_sponsoring": 0,
            "num_sponsored": 0,
            "balances": [
                {
                    "asset_type": "native",
                    "balance": "1000",
                    "selling_liabilities": "0",
                }
            ],
        }

    def get_base_reserve_stroops(self):
        return 5_000_000

    def build_payment(self, **kwargs):
        self.payment = kwargs
        return FakeEnvelope()

    def build_change_trust(self, **kwargs):
        self.trustline = kwargs
        return FakeEnvelope()

    def build_manage_sell_offer(self, **kwargs):
        self.offer = kwargs
        return FakeEnvelope()

    def submit_transaction(self, envelope):
        return {"hash": "tx", "successful": True}


def test_payment_review_keeps_full_issued_asset_identity():
    adapter = FakeAdapter()
    builder = TransactionBuilderService(adapter)
    wallet = Wallet.from_secret(Keypair.random().secret)
    issuer = Keypair.random().public_key

    prepared = builder.build_payment(
        wallet_name="main",
        wallet=wallet,
        destination=Keypair.random().public_key,
        asset=Asset("USDC", issuer),
        amount=Decimal("1"),
        base_fee_stroops=100,
    )

    assert prepared.review.asset == f"USDC:{issuer}"


def test_trustline_review_uses_fresnica_default_and_distinguishes_remove_zero():
    adapter = FakeAdapter()
    builder = TransactionBuilderService(adapter)
    wallet = Wallet.from_secret(Keypair.random().secret)
    asset = Asset("USD", Keypair.random().public_key)

    added = builder.build_trustline(
        wallet_name="main",
        wallet=wallet,
        asset=asset,
        base_fee_stroops=100,
        action="add",
    )
    assert adapter.trustline["limit"] == FRESNICA_TRUSTLINE_LIMIT_TEXT
    assert added.review.limit == FRESNICA_TRUSTLINE_LIMIT_TEXT

    removed = builder.build_trustline(
        wallet_name="main",
        wallet=wallet,
        asset=asset,
        base_fee_stroops=100,
        action="remove",
        limit=Decimal("0"),
    )
    assert adapter.trustline["limit"] == "0"
    assert removed.review.limit is None


def test_pair_aware_cancel_review_preserves_buy_orientation_but_chain_cancel_stays_canonical():
    pair = MarketPair(
        base=Asset("XRP", Keypair.random().public_key),
        counter=Asset("USDC", Keypair.random().public_key),
    )
    offer = OpenOffer(
        offer_id="77",
        selling=pair.counter,
        buying=pair.base,
        selling_amount=Decimal("3.25"),
        price_r=PriceRatio(40, 13),
    )
    adapter = FakeAdapter()
    builder = TransactionBuilderService(adapter)
    transaction = TransactionService(SubmitService(adapter))
    service = OfferService(builder, transaction)
    wallet = Wallet.from_secret(Keypair.random().secret)

    prepared = service.prepare_cancel("main", wallet, offer, pair=pair)

    assert prepared.review.side == "buy"
    assert prepared.review.base_asset == f"XRP:{pair.base.issuer}"
    assert prepared.review.counter_asset == f"USDC:{pair.counter.issuer}"
    assert prepared.review.amount == "10"
    assert prepared.review.price == "0.325"
    assert prepared.review.total == "3.25"
    assert adapter.offer["selling"] == offer.selling
    assert adapter.offer["buying"] == offer.buying
    assert adapter.offer["amount"] == "0"
    assert adapter.offer["price"] == offer.price_r
    assert adapter.offer["offer_id"] == 77
