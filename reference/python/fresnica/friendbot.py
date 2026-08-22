"""Stellar testnet Friendbot integration."""

from urllib.parse import urlencode

import requests

from .errors import NetworkError


class FriendbotService:
    URL = "https://friendbot.stellar.org"

    def __init__(self, url: str | None = None):
        self.url = url or self.URL

    def fund(self, address: str) -> dict:
        try:
            response = requests.get(
                f"{self.url}?{urlencode({'addr': address})}",
                timeout=15,
            )
            response.raise_for_status()
            try:
                return response.json()
            except ValueError:
                return {"status": response.text}
        except requests.RequestException as exc:
            raise NetworkError("Unable to fund testnet account") from exc
