"""Fresnica runtime composition root.

CLI and TUI receive dependencies from here instead of creating them directly.
"""


class Runtime:
    def __init__(self):
        self.wallet_manager = None
        self.balance_service = None
        self.transfer_service = None
        self.transaction_service = None
