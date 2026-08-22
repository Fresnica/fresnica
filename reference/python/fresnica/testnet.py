"""Testnet helpers."""

from .friendbot import FriendbotService


class TestnetService:
    def __init__(self, adapter):
        self.adapter = adapter
        self.friendbot = FriendbotService()

    def fund(self, address: str):
        return self.friendbot.fund(address)
