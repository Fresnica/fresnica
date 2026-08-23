"""Command-line parser for Fresnica command mode."""

import argparse

LANGUAGES = (
    "english",
    "chinese_simplified",
    "chinese_traditional",
    "japanese",
    "korean",
    "spanish",
    "french",
    "italian",
)
NETWORKS = ("mainnet", "testnet")
RESOLUTIONS = ("1m", "5m", "15m", "1h", "1d", "1w")

WALLET_HELP = """Wallet commands:

  Selection / lifecycle
    list                         List local wallets
    use NAME                     Select the default wallet
    delete NAME                  Delete wallet metadata and encrypted secret

  Create / import
    create NAME                  Create a new mnemonic wallet
    import-secret NAME           Import an S... secret
    import-mnemonic NAME         Import a BIP39 mnemonic
    import-watch NAME G...       Add a watch-only account

  Backup / restore
    backup NAME PATH             Export an encrypted portable wallet backup
    restore PATH                 Restore an encrypted wallet backup

  Testnet
    testnet-fund [--wallet NAME] Fund a testnet wallet with Friendbot

Compatibility aliases: watch -> import-watch, fund -> testnet-fund
"""


def build_parser() -> argparse.ArgumentParser:
    parser = argparse.ArgumentParser(prog="fresnica")
    parser.add_argument(
        "--network",
        default="mainnet",
        choices=NETWORKS,
        help="Stellar network context for this invocation",
    )
    sub = parser.add_subparsers(dest="command")

    balance = sub.add_parser("balance", help="Show balances for a wallet")
    balance.add_argument("--wallet", help="Wallet name; defaults to active wallet")
    balance.add_argument("--cached", action="store_true", help="Use local cache only")
    balance.add_argument("--json", action="store_true", dest="as_json")

    history = sub.add_parser("history", help="Show recent account operations")
    history.add_argument("--wallet", help="Wallet name; defaults to active wallet")
    history.add_argument("--limit", type=int, default=20)
    history.add_argument("--cached", action="store_true", help="Use local cache only")

    send = sub.add_parser("send", help="Send a Stellar payment")
    send.add_argument("amount")
    send.add_argument("asset")
    send.add_argument("to_keyword")
    send.add_argument("destination")
    send.add_argument("--wallet", help="Wallet name; defaults to active wallet")
    send.add_argument("--memo")
    send.add_argument("-y", "--yes", action="store_true", help="Skip confirmation")

    info = sub.add_parser("info", help="Show wallet identity and state")
    info.add_argument("--wallet")

    wallet = sub.add_parser(
        "wallet",
        help="Manage wallets",
        description="Manage local wallet identities and watch-only accounts.",
        epilog=WALLET_HELP,
        formatter_class=argparse.RawDescriptionHelpFormatter,
    )
    wallet_sub = wallet.add_subparsers(
        dest="wallet_command",
        required=True,
        metavar="COMMAND",
    )
    wallet_sub.add_parser("list", description="List local wallets")

    use = wallet_sub.add_parser("use", description="Select the default wallet")
    use.add_argument("name")

    delete = wallet_sub.add_parser(
        "delete",
        description="Delete wallet metadata and encrypted signing material",
    )
    delete.add_argument("name")

    create = wallet_sub.add_parser("create", description="Create a new mnemonic wallet")
    create.add_argument("name")
    create.add_argument("--index", type=int, default=0)
    create.add_argument("--language", default="english", choices=LANGUAGES)
    create.add_argument(
        "--strength",
        type=int,
        default=256,
        choices=(128, 160, 192, 224, 256),
    )

    secret = wallet_sub.add_parser(
        "import-secret",
        description="Import a Stellar S... secret",
    )
    secret.add_argument("name")

    mnemonic = wallet_sub.add_parser(
        "import-mnemonic",
        description="Import a BIP39 mnemonic",
    )
    mnemonic.add_argument("name")
    mnemonic.add_argument("--index", type=int, default=0)
    mnemonic.add_argument("--language", choices=LANGUAGES)

    watch = wallet_sub.add_parser(
        "import-watch",
        aliases=["watch"],
        description="Add a watch-only Stellar account",
    )
    watch.add_argument("name")
    watch.add_argument("address")

    fund = wallet_sub.add_parser(
        "testnet-fund",
        aliases=["fund"],
        description="Fund a testnet wallet with Friendbot",
    )
    fund.add_argument("--wallet", help="Wallet name; defaults to active wallet")

    backup = wallet_sub.add_parser(
        "backup",
        description="Write a portable backup containing only encrypted signing material",
    )
    backup.add_argument("name")
    backup.add_argument("path")
    backup.add_argument(
        "--force",
        action="store_true",
        help="Replace an existing backup file",
    )

    restore = wallet_sub.add_parser(
        "restore",
        description="Restore a wallet from a Fresnica encrypted backup",
    )
    restore.add_argument("path")
    restore.add_argument(
        "--name",
        help="Restore under a different local wallet name",
    )

    dex = sub.add_parser("dex", help="Read and trade on the Stellar DEX")
    dex_sub = dex.add_subparsers(dest="dex_command", required=True)

    orderbook = dex_sub.add_parser("orderbook", help="Show an order book")
    orderbook.add_argument("selling")
    orderbook.add_argument("buying")

    offers = dex_sub.add_parser("offers", help="Show current offers for a wallet")
    offers.add_argument("--wallet", help="Wallet name; defaults to active wallet")
    offers.add_argument("--limit", type=int, default=20)
    offers.add_argument("--cached", action="store_true", help="Use local cache only")

    for side in ("buy", "sell"):
        command = dex_sub.add_parser(side, help=f"Create a {side.upper()} limit offer")
        command.add_argument("base")
        command.add_argument("counter")
        command.add_argument("amount", help="Base-asset amount")
        command.add_argument("price", help="Counter units per one base unit")
        command.add_argument("--wallet", help="Wallet name; defaults to active wallet")
        command.add_argument(
            "--allow-trustline",
            action="store_true",
            help="Explicitly approve creating a missing receiving trustline",
        )
        command.add_argument("-y", "--yes", action="store_true", help="Skip confirmation")

    update = dex_sub.add_parser("update", help="Update an existing offer")
    update.add_argument("offer_id")
    update.add_argument("base")
    update.add_argument("counter")
    update.add_argument("amount", help="New base-asset amount")
    update.add_argument("price", help="New counter/base price")
    update.add_argument("--wallet", help="Wallet name; defaults to active wallet")
    update.add_argument("-y", "--yes", action="store_true", help="Skip confirmation")

    cancel = dex_sub.add_parser("cancel", help="Cancel an existing offer")
    cancel.add_argument("offer_id")
    cancel.add_argument("--wallet", help="Wallet name; defaults to active wallet")
    cancel.add_argument("-y", "--yes", action="store_true", help="Skip confirmation")

    fills = dex_sub.add_parser("fills", help="Show recent wallet offer fills")
    fills.add_argument("--wallet", help="Wallet name; defaults to active wallet")
    fills.add_argument("--limit", type=int, default=200)
    fills.add_argument("--cached", action="store_true", help="Use local cache only")

    trades = dex_sub.add_parser("trades", help="Show recent trades for an asset pair")
    trades.add_argument("base")
    trades.add_argument("counter")
    trades.add_argument("--limit", type=int, default=20)
    trades.add_argument("--cached", action="store_true", help="Use local cache only")

    candles = dex_sub.add_parser("candles", help="Show trade aggregations for an asset pair")
    candles.add_argument("base")
    candles.add_argument("counter")
    candles.add_argument("--resolution", default="1h", choices=RESOLUTIONS)
    candles.add_argument("--start", type=int, dest="start_time")
    candles.add_argument("--end", type=int, dest="end_time")
    candles.add_argument("--offset", type=int)
    candles.add_argument("--limit", type=int, default=100)
    candles.add_argument("--cached", action="store_true", help="Use local cache only")
    return parser


def parse_args(argv=None):
    return build_parser().parse_args(argv)


def parse_command(args):
    if not args:
        return {"command": "tui"}
    return {"command": args[0], "args": args[1:]}
