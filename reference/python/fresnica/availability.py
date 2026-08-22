"""Asset availability calculations.

Separates user spendable amount from raw Horizon balances.
"""

from decimal import Decimal


class AvailabilityService:
    def available_balance(self, balance, selling_liabilities="0"):
        """Calculate spendable amount.

        The exact rules depend on asset type and Stellar account state.
        This keeps the calculation outside UI code.
        """
        return Decimal(balance) - Decimal(selling_liabilities)
