"""Balance command."""


def execute_balance(runtime, args, renderer):
    session = runtime.wallet_manager.view(args.wallet)
    services = runtime.services_for(session.record.network)
    if args.cached:
        raw = runtime.datastore.get_balances(
            session.record.network,
            session.record.address,
        )
        # Cached rows intentionally remain raw; reserve-aware available XLM
        # requires fresh account-level reserve fields.
        if args.as_json:
            renderer.console.print_json(
                data={
                    "wallet": session.record.name,
                    "address": session.record.address,
                    "network": session.record.network,
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
