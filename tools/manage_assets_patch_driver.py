from pathlib import Path
import runpy

root = Path(__file__).resolve().parents[1]
py = root / "reference" / "python" / "fresnica"
already = (
    "ASSET_LIST_CATALOGUE" in (py / "asset_catalog.py").read_text(encoding="utf-8")
    and 'yield Static("Manage Assets"' in (py / "tui" / "trustlines.py").read_text(encoding="utf-8")
    and "self.set_focus(offers)" in (py / "tui" / "dex.py").read_text(encoding="utf-8")
)
if already:
    print("manage-assets patch already applied")
else:
    runpy.run_path(str(root / "tools" / "manage_assets_patch.py"), run_name="__main__")
