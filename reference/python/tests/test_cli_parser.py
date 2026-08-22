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
