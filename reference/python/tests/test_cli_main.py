import fresnica.cli.main as cli_main
import fresnica.tui.app as tui_app


def test_network_only_invocation_enters_tui_with_requested_context(monkeypatch):
    created = []
    launched = []

    class FakeRuntime:
        def __init__(self, network="mainnet"):
            self.network = network
            created.append(network)

    monkeypatch.setattr(cli_main, "Runtime", FakeRuntime)
    monkeypatch.setattr(tui_app, "run_tui", lambda runtime: launched.append(runtime))

    assert cli_main.main(["--network", "testnet"]) == 0
    assert created == ["testnet"]
    assert launched[0].network == "testnet"
