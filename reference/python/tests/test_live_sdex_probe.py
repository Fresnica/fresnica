from fresnica.datastore import MemoryDataStore
from fresnica.dex_service import DexService
from fresnica.network import MAINNET
from fresnica.stellar_adapter import StellarAdapter
from fresnica.wallet import Wallet


USDC = "USDC:GA5ZSEJYB37JRC5AVCIA5MOP4RHTM335X2KGX3IHOJAPP5RE34K4KZVN"
USDC_ISSUER = "GA5ZSEJYB37JRC5AVCIA5MOP4RHTM335X2KGX3IHOJAPP5RE34K4KZVN"


def test_live_mainnet_sdex_read_paths():
    adapter = StellarAdapter(MAINNET)
    service = DexService(adapter, MemoryDataStore(), "mainnet")

    orderbook = service.get_orderbook("XLM", USDC)
    assert isinstance(orderbook.get("bids", []), list)
    assert isinstance(orderbook.get("asks", []), list)

    trades = service.get_trades("XLM", USDC, limit=2)
    assert isinstance(trades, list)

    candles = service.get_trade_aggregations("XLM", USDC, resolution="1h", limit=2)
    assert isinstance(candles, list)

    offers = service.get_offers(Wallet.from_address(USDC_ISSUER), limit=1)
    assert isinstance(offers, list)
