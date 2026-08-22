"""Recent operations command."""


def execute_history(runtime, args, renderer):
    session = runtime.wallet_manager.view(args.wallet)
    services = runtime.services_for(session.record.network)
    views = services.history_service.get_views(
        session.wallet,
        limit=args.limit,
        refresh=not args.cached,
    )
    renderer.render_history(session.record, views)
    return views
