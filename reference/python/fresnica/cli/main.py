"""Fresnica command/TUI entry point."""

import sys

from ..errors import FresnicaError, UserCancelled
from ..runtime import Runtime
from .parser import parse_args
from .rich_renderer import RichRenderer


def _run_tui(runtime):
    from .commands.tui import run

    return run(runtime)


def main(argv=None, runtime=None) -> int:
    argv = list(sys.argv[1:] if argv is None else argv)

    if not argv:
        _run_tui(runtime or Runtime())
        return 0

    args = parse_args(argv)
    runtime = runtime or Runtime(network=args.network)

    if args.command is None:
        _run_tui(runtime)
        return 0

    renderer = RichRenderer()
    try:
        if args.command == "balance":
            from .commands.balance import execute_balance

            execute_balance(runtime, args, renderer)
        elif args.command == "history":
            from .commands.history import execute_history

            execute_history(runtime, args, renderer)
        elif args.command == "send":
            from .commands.send import execute_send

            execute_send(runtime, args, renderer)
        elif args.command == "info":
            from .commands.info import execute_info

            execute_info(runtime, args, renderer)
        elif args.command == "wallet":
            from .commands.wallet import execute_wallet

            execute_wallet(runtime, args, renderer)
        elif args.command == "contact":
            from .commands.contact import execute_contact

            execute_contact(runtime, args, renderer)
        elif args.command == "trust":
            from .commands.trust import execute_trust

            execute_trust(runtime, args, renderer)
        elif args.command == "dex":
            from .commands.dex import execute_dex

            execute_dex(runtime, args, renderer)
        return 0
    except UserCancelled as exc:
        renderer.error(str(exc))
        return 2
    except (FresnicaError, ValueError) as exc:
        renderer.error(str(exc), getattr(exc, "details", None))
        return 1


if __name__ == "__main__":
    raise SystemExit(main())
