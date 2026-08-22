"""Fresnica CLI entry point.

Command mode and TUI mode share the same runtime.

fresnica           -> TUI
fresnica <command> -> command execution
"""


def main(argv=None):
    from .parser import parse_args

    args = parse_args(argv)

    if args.command is None:
        from ..tui.app import run_tui
        return run_tui()

    return args.execute()
