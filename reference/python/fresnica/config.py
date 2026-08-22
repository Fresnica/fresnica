"""Runtime configuration."""

from dataclasses import dataclass


@dataclass
class Config:
    default_network: str = "testnet"

    @classmethod
    def development(cls):
        return cls(default_network="testnet")
