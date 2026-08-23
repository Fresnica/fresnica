"""User account trade normalization and offer-level fill aggregation."""

from dataclasses import replace
from decimal import Decimal, InvalidOperation

from .models import AccountTrade, AccountTradeSegment, Asset, MarketPair, PriceRatio


def account_trade_from_horizon(raw: dict, address: str) -> AccountTrade:
    try:
        price = raw["price"]
        price_r = PriceRatio(int(price["n"]), int(price["d"]))
        base_amount = Decimal(str(raw["base_amount"]))
        counter_amount = Decimal(str(raw["counter_amount"]))
    except (KeyError, TypeError, ValueError, InvalidOperation) as exc:
        raise ValueError("Invalid Horizon trade record") from exc

    base_account = raw.get("base_account")
    counter_account = raw.get("counter_account")
    if base_account == address:
        side = "sell"
    elif counter_account == address:
        side = "buy"
    else:
        side = "sell" if raw.get("base_is_seller") else "buy"

    return AccountTrade(
        trade_id=str(raw.get("id", raw.get("paging_token", ""))),
        pair=MarketPair(
            base=_trade_asset(raw, "base"),
            counter=_trade_asset(raw, "counter"),
        ),
        base_amount=base_amount,
        counter_amount=counter_amount,
        price_r=price_r,
        side=side,
        time=raw.get("ledger_close_time"),
        paging_token=str(raw["paging_token"]) if raw.get("paging_token") is not None else None,
        base_account=base_account,
        counter_account=counter_account,
        base_offer_id=_text_or_none(raw.get("base_offer_id")),
        counter_offer_id=_text_or_none(raw.get("counter_offer_id")),
        transaction_hash=_text_or_none(raw.get("transaction_hash")),
        raw=raw,
    )


def account_trade_for_pair(
    trade: AccountTrade,
    pair: MarketPair,
    address: str,
) -> AccountTrade | None:
    if trade.pair == pair:
        return replace(trade, side=_side_for_account(trade, address))
    if trade.pair.base != pair.counter or trade.pair.counter != pair.base:
        return None

    direct_side = _side_for_account(trade, address)
    return replace(
        trade,
        pair=pair,
        base_amount=trade.counter_amount,
        counter_amount=trade.base_amount,
        price_r=PriceRatio(trade.price_r.d, trade.price_r.n),
        side="sell" if direct_side == "buy" else "buy",
        base_account=trade.counter_account,
        counter_account=trade.base_account,
        base_offer_id=trade.counter_offer_id,
        counter_offer_id=trade.base_offer_id,
    )


def account_trade_segment_for_pair(
    segment: AccountTradeSegment,
    pair: MarketPair,
) -> AccountTradeSegment | None:
    """Project one aggregated fill segment into an explicit market orientation."""
    if segment.pair == pair:
        return segment
    if segment.pair.base != pair.counter or segment.pair.counter != pair.base:
        return None

    side = "sell" if segment.side == "buy" else "buy"
    price_r = PriceRatio(segment.price_r.d, segment.price_r.n)
    return replace(
        segment,
        segment_key=_segment_identity(pair, side, price_r, segment.user_offer_id),
        pair=pair,
        side=side,
        base_amount=segment.counter_amount,
        counter_amount=segment.base_amount,
        price_r=price_r,
    )


def compress_account_trades(
    records: list[AccountTrade],
    address: str,
) -> list[AccountTradeSegment]:
    """Merge only consecutive fills from the same identified user offer."""

    result: list[AccountTradeSegment] = []
    for trade in records:
        segment = _segment_from_trade(trade, address)
        previous = result[-1] if result else None
        if previous is not None and _segments_can_merge(previous, segment):
            result[-1] = _merge_segments(previous, segment)
        else:
            result.append(segment)
    return result


def _segment_from_trade(trade: AccountTrade, address: str) -> AccountTradeSegment:
    offer_id = user_offer_id(trade, address)
    return AccountTradeSegment(
        segment_key=_segment_key(trade, offer_id),
        pair=trade.pair,
        side=trade.side,
        base_amount=trade.base_amount,
        counter_amount=trade.counter_amount,
        price_r=trade.price_r,
        user_offer_id=offer_id,
        trade_count=1,
        first_time=trade.time,
        last_time=trade.time,
        first_trade_id=trade.trade_id,
        last_trade_id=trade.trade_id,
        raw=[trade.raw],
    )


def _segments_can_merge(
    first: AccountTradeSegment,
    last: AccountTradeSegment,
) -> bool:
    # Missing offer IDs occur for non-orderbook trades such as AMM activity.
    # Pair/side/price alone is not sufficient evidence that two fills belong
    # to one user order.
    return (
        first.user_offer_id is not None
        and last.user_offer_id is not None
        and first.segment_key == last.segment_key
    )


def _merge_segments(
    first: AccountTradeSegment,
    last: AccountTradeSegment,
) -> AccountTradeSegment:
    return AccountTradeSegment(
        segment_key=first.segment_key,
        pair=first.pair,
        side=first.side,
        base_amount=first.base_amount + last.base_amount,
        counter_amount=first.counter_amount + last.counter_amount,
        price_r=first.price_r,
        user_offer_id=first.user_offer_id,
        trade_count=first.trade_count + last.trade_count,
        first_time=first.first_time,
        last_time=last.last_time,
        first_trade_id=first.first_trade_id,
        last_trade_id=last.last_trade_id,
        raw=[*first.raw, *last.raw],
    )


def user_offer_id(trade: AccountTrade, address: str) -> str | None:
    if trade.base_account == address:
        return trade.base_offer_id
    if trade.counter_account == address:
        return trade.counter_offer_id
    return None


def _segment_key(trade: AccountTrade, offer_id: str | None) -> str:
    return _segment_identity(trade.pair, trade.side, trade.price_r, offer_id)


def _segment_identity(
    pair: MarketPair,
    side: str,
    price_r: PriceRatio,
    offer_id: str | None,
) -> str:
    return "|".join(
        (
            _asset_key(pair.base),
            _asset_key(pair.counter),
            side,
            f"{price_r.n}:{price_r.d}",
            offer_id or "",
        )
    )


def _side_for_account(trade: AccountTrade, address: str):
    if trade.base_account == address:
        return "sell"
    if trade.counter_account == address:
        return "buy"
    return trade.side


def _trade_asset(raw: dict, prefix: str) -> Asset:
    if raw.get(f"{prefix}_asset_type") == "native":
        return Asset("XLM")
    code = raw.get(f"{prefix}_asset_code")
    issuer = raw.get(f"{prefix}_asset_issuer")
    if not code or not issuer:
        raise ValueError("Invalid Horizon trade asset")
    return Asset(str(code), str(issuer))


def _asset_key(asset: Asset) -> str:
    return "XLM" if asset.is_native else f"{asset.code}:{asset.issuer}"


def _text_or_none(value) -> str | None:
    return None if value is None else str(value)
