"""Persistent user-interface preferences."""

from dataclasses import asdict, dataclass
import json
from pathlib import Path


@dataclass
class UserSettings:
    show_zero_balances: bool = False
    hide_suspicious_claimables: bool = False
    use_local_time: bool = True
    keep_full_history: bool = False
    theme: str | None = None


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
        theme = raw.get("theme")
        return UserSettings(
            show_zero_balances=bool(raw.get("show_zero_balances", False)),
            # Older builds used show_dust_activity=False to hide claimables by
            # default. Do not preserve that over-broad behavior: suspicious
            # claimables are visible by default and users may explicitly hide
            # only the narrowly classified entries below.
            hide_suspicious_claimables=bool(
                raw.get("hide_suspicious_claimables", False)
            ),
            use_local_time=bool(raw.get("use_local_time", True)),
            keep_full_history=bool(raw.get("keep_full_history", False)),
            theme=str(theme) if theme else None,
        )

    def save(self, settings: UserSettings) -> None:
        self.path.parent.mkdir(parents=True, exist_ok=True)
        temporary = self.path.with_suffix(self.path.suffix + ".tmp")
        temporary.write_text(
            json.dumps(asdict(settings), indent=2, sort_keys=True) + "\n",
            encoding="utf-8",
        )
        temporary.replace(self.path)
