"""Balance command."""

from ..context import require_wallet_network


def execute_balance(runtime, args, renderer):
    session = runtime.wallet_manager.view(args.wallet)
    require_wallet_network(session.record, runtime.network)
    services = runtime.services_for()
    if args.cached:
        raw = runtime.datastore.get_balances(runtime.network, session.record.address)
        if args.as_json:
            renderer.console.print_json(
                data={
                    "wallet": session.record.name,
                    "address": session.record.address,
                    "network": runtime.network,
                    "balances": raw,
                    "cached": True,
                }
            )
            return raw
        renderer.console.print(raw)
        return raw

    views = services.balance_service.get_views(session.wallet)
    if args.as_json:
        renderer.render_balance_json(session.record, views)
    else:
        renderer.render_balance(session.record, views)
    return views
