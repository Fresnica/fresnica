from types import SimpleNamespace

from stellar_sdk import Keypair

from fresnica.cli.commands.wallet import execute_wallet
from fresnica.manager import WalletManager
from fresnica.storage import MemoryWalletStorage


class RecordingRenderer:
    def __init__(self):
        self.messages = []

    def success(self, message):
        self.messages.append(message)


def test_cli_backup_and_restore_round_trip_without_secret_prompt(tmp_path):
    manager = WalletManager(MemoryWalletStorage())
    manager.add_watch("observer", Keypair.random().public_key, network="testnet")
    runtime = SimpleNamespace(wallet_manager=manager, network="testnet")
    renderer = RecordingRenderer()
    backup_path = tmp_path / "observer.json"

    backup = execute_wallet(
        runtime,
        SimpleNamespace(
            wallet_command="backup",
            name="observer",
            path=str(backup_path),
            force=False,
        ),
        renderer,
        secret_input=lambda prompt: (_ for _ in ()).throw(
            AssertionError(f"unexpected secret prompt: {prompt}")
        ),
    )

    manager.delete("observer")
    restored = execute_wallet(
        runtime,
        SimpleNamespace(
            wallet_command="restore",
            path=str(backup_path),
            name="copy",
        ),
        renderer,
        secret_input=lambda prompt: (_ for _ in ()).throw(
            AssertionError(f"unexpected secret prompt: {prompt}")
        ),
    )

    assert backup == backup_path
    assert restored.name == "copy"
    assert restored.network == "testnet"
    assert manager.get_record("copy").address == restored.address
    assert "Encrypted backup" in renderer.messages[0]
    assert "original wallet password" in renderer.messages[1]
