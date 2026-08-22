"""Balance command."""


def execute_balance(context):
    return context.balance_service.get_balances(
        context.wallet
    )
