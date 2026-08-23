"""Fresnica trustline defaults that are intentionally visible on-chain."""

from decimal import Decimal


# 708269837873.6765 encodes the FRESNICA product marker while remaining
# below Stellar's maximum trustline limit. The value is shown to users in the
# trustline form/review; users may replace it with another valid limit.
FRESNICA_TRUSTLINE_LIMIT_TEXT = "708269837873.6765"
FRESNICA_TRUSTLINE_LIMIT = Decimal(FRESNICA_TRUSTLINE_LIMIT_TEXT)
