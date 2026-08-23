from types import SimpleNamespace

from fresnica.cli.commands._post_submit import refresh_after_submit
from fresnica.errors import NetworkError


class FakeBalance:
    def __init__(self, fail=False):
        self.calls = 0
        self.fail = fail

    def get_account(self, wallet, refresh=True):
        assert refresh is True
        self.calls += 1
        if self.fail:
            raise NetworkError("offline")


class FakeHistory:
    def __init__(self):
        self.calls = 0

    def sync_recent(self, wallet):
        self.calls += 1


class FakeDex:
    def __init__(self):
        self.offers = 0
        self.fills = 0

    def get_open_offers(self, wallet, limit=200, refresh=True):
        assert limit == 200
        assert refresh is True
        self.offers += 1

    def get_account_trade_segments(self, wallet, limit=200, refresh=True):
        assert limit == 200
        assert refresh is True
        self.fills += 1


def test_post_submit_refreshes_wallet_and_dex_state():
    balance = FakeBalance()
    history = FakeHistory()
    dex = FakeDex()
    services = SimpleNamespace(
        balance_service=balance,
        history_service=history,
        dex_service=dex,
    )

    refresh_after_submit(services, object(), include_dex=True)

    assert balance.calls == 1
    assert history.calls == 1
    assert dex.offers == 1
    assert dex.fills == 1


def test_post_submit_refresh_failure_does_not_rewrite_confirmed_success():
    balance = FakeBalance(fail=True)
    history = FakeHistory()
    services = SimpleNamespace(balance_service=balance, history_service=history)

    refresh_after_submit(services, object())

    assert balance.calls == 1
    assert history.calls == 1
