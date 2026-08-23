from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
path = ROOT / "reference" / "python" / "tests" / "test_tui.py"
text = path.read_text(encoding="utf-8")
old = '''            await pilot.press("m")\n            await _settle(pilot, 8)\n            assert runtime.history_services["testnet"].older == 1\n'''
new = '''            await pilot.press("m")\n            await _settle(pilot, 8)\n            # Older reveals the retained local cache; it no longer starts a\n            # separate Horizon backfill in the default 2,000-operation mode.\n            assert runtime.history_services["testnet"].older == 0\n            assert "No more cached activity" in str(\n                app.screen.query_one("#history-status", Static).render()\n            )\n'''
if text.count(old) != 1:
    raise RuntimeError("expected the legacy History Older assertion exactly once")
path.write_text(text.replace(old, new, 1), encoding="utf-8")
Path(__file__).unlink()
