from types import SimpleNamespace

from fresnica.cli.commands.dex import execute_dex
from fresnica.cli.parser import parse_args


class FakeDexService:
    def __init__(self):
        self.calls = []

    def get_orderbook(self, selling, buying):
        self.calls.append(("orderbook", selling, buying))
        return {"bids": [], "asks": []}

    def get_trades(self, base, counter, limit=20, refresh=True):
        self.calls.append(("trades", base, counter, limit, refresh))
        return []

    def get_trade_aggregations(self, base, counter, **kwargs):
        self.calls.append(("candles", base, counter, kwargs))
        return []


class FakeRenderer:
    def __init__(self):
        self.rendered = None

    def render_orderbook(self, *args):
        self.rendered = ("orderbook", args)

    def render_trades(self, *args):
        self.rendered = ("trades", args)

    def render_trade_aggregations(self, *args):
        self.rendered = ("candles", args)


def _runtime(service):
    return SimpleNamespace(
        network="testnet",
        services_for=lambda: SimpleNamespace(dex_service=service),
    )


def test_dex_parser_and_dispatch():
    args = parse_args(["--network", "testnet", "dex", "orderbook", "XLM", "USD:GISSUER"])
    assert args.command == "dex"
    assert args.dex_command == "orderbook"

    service = FakeDexService()
    renderer = FakeRenderer()
    execute_dex(_runtime(service), args, renderer)
    assert service.calls == [("orderbook", "XLM", "USD:GISSUER")]
    assert renderer.rendered[0] == "orderbook"


def test_dex_trade_and_candle_options():
    service = FakeDexService()
    renderer = FakeRenderer()

    trades = parse_args(
        ["--network", "testnet", "dex", "trades", "XLM", "USD:GISSUER", "--limit", "7", "--cached"]
    )
    execute_dex(_runtime(service), trades, renderer)
    assert service.calls[-1] == ("trades", "XLM", "USD:GISSUER", 7, False)

    candles = parse_args(
        [
            "--network",
            "testnet",
            "dex",
            "candles",
            "XLM",
            "USD:GISSUER",
            "--resolution",
            "15m",
            "--start",
            "1000",
            "--end",
            "2000",
            "--limit",
            "12",
        ]
    )
    execute_dex(_runtime(service), candles, renderer)
    _, base, counter, kwargs = service.calls[-1]
    assert (base, counter) == ("XLM", "USD:GISSUER")
    assert kwargs["resolution"] == "15m"
    assert kwargs["start_time"] == 1000
    assert kwargs["end_time"] == 2000
    assert kwargs["limit"] == 12
