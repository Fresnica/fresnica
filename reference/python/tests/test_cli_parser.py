import pytest

from fresnica.cli.parser import parse_args


def test_send_sentence_shape():
    args = parse_args(["send", "100", "xlm", "to", "GDEST"])
    assert args.command == "send"
    assert args.amount == "100"
    assert args.asset == "xlm"
    assert args.to_keyword == "to"
    assert args.destination == "GDEST"


def test_wallet_create_chinese_mnemonic_option():
    args = parse_args(
        ["wallet", "create", "main", "--language", "chinese_simplified"]
    )
    assert args.wallet_command == "create"
    assert args.language == "chinese_simplified"


def test_global_testnet_context_survives_wallet_subcommand():
    args = parse_args(["--network", "testnet", "wallet", "create", "demo"])
    assert args.network == "testnet"
    assert args.wallet_command == "create"


def test_wallet_testnet_fund_uses_global_network_context():
    args = parse_args(["--network", "testnet", "wallet", "testnet-fund"])
    assert args.network == "testnet"
    assert args.wallet_command == "testnet-fund"
    assert args.wallet is None


def test_wallet_compatibility_aliases_still_parse():
    watch = parse_args(["wallet", "watch", "observer", "GDEST"])
    fund = parse_args(["--network", "testnet", "wallet", "fund"])
    assert watch.wallet_command == "watch"
    assert fund.wallet_command == "fund"


def test_wallet_import_watch_is_canonical_command():
    args = parse_args(["--network", "mainnet", "wallet", "import-watch", "observer", "GDEST"])
    assert args.wallet_command == "import-watch"
    assert args.name == "observer"
    assert args.address == "GDEST"


def test_wallet_help_is_grouped(capsys):
    with pytest.raises(SystemExit) as exc:
        parse_args(["wallet", "--help"])
    assert exc.value.code == 0
    output = capsys.readouterr().out
    assert "Selection / lifecycle" in output
    assert "Create / import" in output
    assert "Testnet" in output
    assert "import-watch NAME G..." in output
    assert "testnet-fund" in output
