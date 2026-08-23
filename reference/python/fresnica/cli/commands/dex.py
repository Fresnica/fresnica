"""SDEX command handlers."""

from decimal import Decimal, InvalidOperation
from getpass import getpass

from ...errors import UserCancelled, WatchOnlyError
from ...models import Asset, MarketPair, OfferIntent
from ...network import get_network
from ...offer_service import offer_view_for_pair
from ...presentation import offer_outcome_summary
from ..context import require_wallet_network
from ._post_submit import refresh_after_submit


def execute_dex(
    runtime,
    args,
    renderer,
    password_provider=getpass,
    confirm_provider=None,
):
    services = runtime.services_for()
    service = services.dex_service

    if args.dex_command == "orderbook":
        orderbook = service.get_orderbook(args.selling, args.buying)
        renderer.render_orderbook(args.selling, args.buying, orderbook, runtime.network)
        return orderbook

    if args.dex_command == "offers":
        record, session = _view_wallet(runtime, args.wallet)
        offers = service.get_offers(
            session.wallet,
            limit=args.limit,
            refresh=not args.cached,
        )
        renderer.render_offers(record, offers)
        return offers

    if args.dex_command == "fills":
        record, session = _view_wallet(runtime, args.wallet)
        segments = service.get_account_trade_segments(
            session.wallet,
            limit=args.limit,
            refresh=not args.cached,
        )
        renderer.render_account_trade_segments(record, segments)
        return segments

    if args.dex_command in ("buy", "sell", "update", "cancel"):
        return _execute_offer_write(
            runtime,
            args,
            renderer,
            password_provider=password_provider,
            confirm_provider=confirm_provider,
        )

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


def _execute_offer_write(
    runtime,
    args,
    renderer,
    password_provider,
    confirm_provider,
):
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
        offer_service = services.offer_service

        if args.dex_command in ("buy", "sell"):
            pair = _pair(args.base, args.counter)
            intent = OfferIntent(
                pair=pair,
                side=args.dex_command,
                amount=_decimal_arg(args.amount, "amount"),
                price=_decimal_arg(args.price, "price"),
            )
            prepared = offer_service.prepare_create(
                record.name,
                session.wallet,
                intent,
                allow_trustline=args.allow_trustline,
            )

        elif args.dex_command == "update":
            pair = _pair(args.base, args.counter)
            offer = _find_offer(services.dex_service, session.wallet, args.offer_id)
            view = offer_view_for_pair(offer, pair)
            if view is None:
                raise ValueError(
                    f"Offer {args.offer_id} does not belong to {args.base}/{args.counter}"
                )
            intent = OfferIntent(
                pair=pair,
                side=view.side,
                amount=_decimal_arg(args.amount, "amount"),
                price=_decimal_arg(args.price, "price"),
            )
            prepared = offer_service.prepare_update(
                record.name,
                session.wallet,
                offer,
                intent,
            )

        else:
            offer = _find_offer(services.dex_service, session.wallet, args.offer_id)
            prepared = offer_service.prepare_cancel(record.name, session.wallet, offer)

        renderer.render_offer_review(prepared.review)
        confirmed = args.yes
        if not confirmed:
            confirmed = (confirm_provider or renderer.confirm)()
        if not confirmed:
            raise UserCancelled("Transaction cancelled")

        offer_service.sign(session.wallet, prepared)
        result = offer_service.submit(prepared)
        renderer.render_result(result, get_network(runtime.network))
        outcome = offer_outcome_summary(getattr(result, "offer_outcome", None))
        if outcome:
            renderer.success(f"Offer result: {outcome}")
        refresh_after_submit(services, session.wallet, include_dex=True)
        return result
    finally:
        manager.lock()


def _find_offer(service, wallet, offer_id: str):
    return service.get_open_offer(wallet, str(offer_id))


def _pair(base: str, counter: str) -> MarketPair:
    return MarketPair(base=Asset.parse(base), counter=Asset.parse(counter))


def _decimal_arg(value: str, label: str) -> Decimal:
    try:
        return Decimal(value)
    except InvalidOperation as exc:
        raise ValueError(f"Invalid offer {label}: {value}") from exc


def _view_wallet(runtime, wallet_name):
    manager = runtime.wallet_manager
    record = manager.get_record(wallet_name)
    require_wallet_network(record, runtime.network)
    return record, manager.view(record.name)
