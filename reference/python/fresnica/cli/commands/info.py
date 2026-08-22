"""Wallet information command."""


def run(runtime):
    wallet = runtime.wallet_manager.current()
    return wallet
