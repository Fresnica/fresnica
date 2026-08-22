"""Fresnica CLI command parser.

CLI is an interface layer only. Business logic belongs to services.
"""


def parse_command(args):
    if not args:
        return {"command": "tui"}

    command = args[0]

    return {
        "command": command,
        "args": args[1:],
    }
