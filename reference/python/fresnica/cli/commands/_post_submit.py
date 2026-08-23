"""Best-effort account cache refresh after a confirmed one-shot CLI write."""


def refresh_after_submit(services, wallet, *, include_dex: bool = False) -> None:
    """Refresh derived local state without turning a confirmed tx into a failure."""
    actions = []

    balance_service = getattr(services, "balance_service", None)
    get_account = getattr(balance_service, "get_account", None)
    if callable(get_account):
        actions.append(lambda: get_account(wallet, refresh=True))

    history_service = getattr(services, "history_service", None)
    sync_recent = getattr(history_service, "sync_recent", None)
    if callable(sync_recent):
        actions.append(lambda: sync_recent(wallet))

    if include_dex:
        dex_service = getattr(services, "dex_service", None)
        get_open_offers = getattr(dex_service, "get_open_offers", None)
        if callable(get_open_offers):
            actions.append(lambda: get_open_offers(wallet, limit=200, refresh=True))
        get_segments = getattr(dex_service, "get_account_trade_segments", None)
        if callable(get_segments):
            actions.append(lambda: get_segments(wallet, limit=200, refresh=True))

    for action in actions:
        try:
            action()
        except Exception:
            # The ledger write is already confirmed. Derived caches are disposable
            # and will be rebuilt on the next normal read/refresh.
            pass
