"""Small shared result contract for bounded chain synchronization."""

from dataclasses import dataclass


@dataclass(frozen=True)
class SyncResult:
    fetched_count: int
    caught_up: bool
