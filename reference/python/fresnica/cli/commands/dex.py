"""Read-only SDEX command handlers."""

from ..context import require_wallet_network


def execute_dex(runtime, args, renderer):
    service = runtime.services_for().dex_service

    if args.dex_command == "orderbook":
        orderbook = service.get_orderbook(args.selling, args.buying)
        renderer.render_orderbook(args.selling, args.buying, orderbook, runtime.network)
        return orderbook

    if args.dex_command == "offers":
        manager = runtime.wallet_manager
        record = manager.get_record(args.wallet)
        require_wallet_network(record, runtime.network)
        session = manager.view(record.name)
        offers = service.get_offers(
            session.wallet,
            limit=args.limit,
            refresh=not args.cached,
        )
        renderer.render_offers(record, offers)
        return offers

    if args.dex_command == "trades":
        trades = service.get_trades(
            args.base,
            args.counter,
            limit=args.limit,
            refresh=not args.cached,
        )
        renderer.render_trades(args.base, args.counter, trades, runtime.network)
        return trades

    if args.dex_command == "candles":
        aggregations = service.get_trade_aggregations(
            args.base,
            args.counter,
            resolution=args.resolution,
            start_time=args.start_time,
            end_time=args.end_time,
            offset=args.offset,
            limit=args.limit,
            refresh=not args.cached,
        )
        renderer.render_trade_aggregations(
            args.base,
            args.counter,
            args.resolution,
            aggregations,
            runtime.network,
        )
        return aggregations

    raise ValueError(f"Unknown dex command: {args.dex_command}")
