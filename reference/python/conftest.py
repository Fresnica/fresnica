"""Temporary CI bootstrap for the architecture refactor branch."""

from pathlib import Path
import runpy

runpy.run_path(
    str(Path(__file__).resolve().parents[2] / "tools" / "anchor_history_patch.py"),
    run_name="__main__",
)
