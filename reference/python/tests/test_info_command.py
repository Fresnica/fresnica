from argparse import Namespace

from fresnica.cli.commands.info import execute_info
from fresnica.manager import WalletManager
from fresnica.storage import MemoryWalletStorage


class Renderer:
    def __init__(self):
        self.record = None

    def render_info(self, record):
        self.record = record


class Runtime:
    def __init__(self):
        self.wallet_manager = WalletManager(MemoryWalletStorage())


def test_info_uses_default_wallet():
    runtime = Runtime()
    runtime.wallet_manager.add_watch(
        "observer",
        "GAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAWHF",
    )
    renderer = Renderer()

    record = execute_info(runtime, Namespace(wallet=None), renderer)

    assert record.name == "observer"
    assert renderer.record is record
