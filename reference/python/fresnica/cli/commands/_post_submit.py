"""Best-effort account cache refresh after a confirmed one-shot CLI write."""

from ...errors import FresnicaError


def refresh_after_submit(services, wallet, *, include_dex: bool = False) -> None:
    """Refresh derived local state without turning a confirmed tx into a failure."""
    actions = []

    balance_service = getattr(services, "balance_service", None)
    if balance_service is not None:
        actions.append(lambda: balance_service.get_account(wallet, refresh=True))

    history_service = getattr(services, "history_service", None)
    if history_service is not None:
        actions.append(lambda: history_service.sync_recent(wallet))

    if include_dex:
        dex_service = getattr(services, "dex_service", None)
        if dex_service is not None:
            actions.extend(
                [
                    lambda: dex_service.get_open_offers(wallet, limit=200, refresh=True),
                    lambda: dex_service.get_account_trade_segments(
                        wallet, limit=200, refresh=True
                    ),
                ]
            )

    for action in actions:
        try:
            action()
        except FresnicaError:
            # The ledger write is already confirmed. Cache refresh is recoverable
            # on the next read and must not rewrite success as transaction failure.
            pass
