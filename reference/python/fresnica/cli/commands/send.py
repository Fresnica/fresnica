"""One-shot send command."""

from getpass import getpass

from ...errors import UserCancelled, WatchOnlyError
from ...network import get_network
from ..context import require_wallet_network
from ._post_submit import refresh_after_submit


def execute_send(
    runtime,
    args,
    renderer,
    password_provider=getpass,
    confirm_provider=None,
):
    if args.to_keyword.lower() != "to":
        raise ValueError("Usage: fresnica send AMOUNT ASSET to DESTINATION")

    manager = runtime.wallet_manager
    record = manager.get_record(args.wallet)
    require_wallet_network(record, runtime.network)
    if record.watch_only:
        raise WatchOnlyError(f'Wallet "{record.name}" is watch-only')

    services = runtime.services_for()
    pending = getattr(services, "pending_transaction_service", None)
    if pending is not None:
        pending.ensure_clear(record.address)

    current = manager.current()
    if current is not None and current.record.name == record.name:
        session = current
    else:
        password = password_provider(f'Wallet password for "{record.name}": ')
        session = manager.unlock(record.name, password)

    try:
        prepared = services.transfer_service.prepare(
            wallet_name=record.name,
            wallet=session.wallet,
            destination=args.destination,
            asset=args.asset,
            amount=args.amount,
            memo=args.memo,
        )
        renderer.render_review(prepared.review)

        confirmed = args.yes
        if not confirmed:
            confirmed = (confirm_provider or renderer.confirm)()
        if not confirmed:
            raise UserCancelled("Transaction cancelled")

        services.transfer_service.sign(session.wallet, prepared)
        result = services.transfer_service.submit(prepared)
        renderer.render_result(result, get_network(runtime.network))
        refresh_after_submit(services, session.wallet)
        return result
    finally:
        manager.lock()
