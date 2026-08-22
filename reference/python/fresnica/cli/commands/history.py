"""Recent account activity command."""

from ..context import require_wallet_network


def execute_history(runtime, args, renderer):
    session = runtime.wallet_manager.view(args.wallet)
    require_wallet_network(session.record, runtime.network)
    services = runtime.services_for()
    views = services.history_service.get_activity_views(
        session.wallet,
        limit=args.limit,
        refresh=not args.cached,
    )
    renderer.render_history(session.record, views)
    return views
