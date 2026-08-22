"""Persistent user-interface preferences."""

from dataclasses import asdict, dataclass
import json
from pathlib import Path


@dataclass
class UserSettings:
    show_zero_balances: bool = False


class SettingsStore:
    def __init__(self, path: str | Path):
        self.path = Path(path).expanduser()

    def load(self) -> UserSettings:
        if not self.path.exists():
            return UserSettings()
        try:
            raw = json.loads(self.path.read_text(encoding="utf-8"))
        except (OSError, ValueError, TypeError):
            return UserSettings()
        return UserSettings(show_zero_balances=bool(raw.get("show_zero_balances", False)))

    def save(self, settings: UserSettings) -> None:
        self.path.parent.mkdir(parents=True, exist_ok=True)
        temporary = self.path.with_suffix(self.path.suffix + ".tmp")
        temporary.write_text(
            json.dumps(asdict(settings), indent=2, sort_keys=True) + "\n",
            encoding="utf-8",
        )
        temporary.replace(self.path)
