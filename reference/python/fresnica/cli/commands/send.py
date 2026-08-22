"""One-shot send command."""

from getpass import getpass

from ...errors import UserCancelled, WatchOnlyError
from ...network import get_network


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
    if record.watch_only:
        raise WatchOnlyError(f'Wallet "{record.name}" is watch-only')

    current = manager.current()
    if current is not None and current.record.name == record.name:
        session = current
    else:
        password = password_provider(f'Wallet password for "{record.name}": ')
        session = manager.unlock(record.name, password)

    try:
        services = runtime.services_for(record.network)
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
        renderer.render_result(result, get_network(record.network))
        return result
    finally:
        # Command mode is intentionally ephemeral: never leave signing material
        # attached after a one-shot command exits.
        manager.lock()
