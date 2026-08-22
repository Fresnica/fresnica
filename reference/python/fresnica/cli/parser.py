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

    wallet = sub.add_parser("wallet", help="Manage wallets")
    wallet_sub = wallet.add_subparsers(dest="wallet_command", required=True)
    wallet_sub.add_parser("list", help="List wallets")

    use = wallet_sub.add_parser("use", help="Select default wallet")
    use.add_argument("name")

    watch = wallet_sub.add_parser("watch", help="Add a watch-only wallet")
    watch.add_argument("name")
    watch.add_argument("address")

    secret = wallet_sub.add_parser("import-secret", help="Import an S... secret")
    secret.add_argument("name")

    mnemonic = wallet_sub.add_parser("import-mnemonic", help="Import a mnemonic")
    mnemonic.add_argument("name")
    mnemonic.add_argument("--index", type=int, default=0)
    mnemonic.add_argument("--language", choices=LANGUAGES)

    create = wallet_sub.add_parser("create", help="Create a new mnemonic wallet")
    create.add_argument("name")
    create.add_argument("--index", type=int, default=0)
    create.add_argument("--language", default="english", choices=LANGUAGES)
    create.add_argument(
        "--strength",
        type=int,
        default=256,
        choices=(128, 160, 192, 224, 256),
    )

    fund = wallet_sub.add_parser("fund", help="Fund a testnet wallet with Friendbot")
    fund.add_argument("--wallet", help="Wallet name; defaults to active wallet")

    delete = wallet_sub.add_parser("delete", help="Delete a wallet")
    delete.add_argument("name")

    dex = sub.add_parser("dex", help="Read Stellar DEX market data")
    dex_sub = dex.add_subparsers(dest="dex_command", required=True)

    orderbook = dex_sub.add_parser("orderbook", help="Show an order book")
    orderbook.add_argument("selling")
    orderbook.add_argument("buying")

    offers = dex_sub.add_parser("offers", help="Show current offers for a wallet")
    offers.add_argument("--wallet", help="Wallet name; defaults to active wallet")
    offers.add_argument("--limit", type=int, default=20)
    offers.add_argument("--cached", action="store_true", help="Use local cache only")

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
