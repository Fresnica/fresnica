from pathlib import Path
from types import SimpleNamespace

import pytest

from fresnica.rust_core_client import RustCoreClient, RustCoreUnavailableError


def _make_executable(path: Path) -> None:
    path.write_text("placeholder", encoding="utf-8")
    path.chmod(path.stat().st_mode | 0o100)


def test_relative_core_binary_is_resolved_before_execution(tmp_path, monkeypatch):
    binary = tmp_path / "fresnica-core"
    _make_executable(binary)
    monkeypatch.chdir(tmp_path)
    client = RustCoreClient("./fresnica-core")
    monkeypatch.chdir(tmp_path.parent)

    calls = []

    def fake_run(args, **kwargs):
        calls.append((args, kwargs))
        return SimpleNamespace(
            stdout=b'{"ok":true,"protocol_version":1,"result":{"core_version":"test","protocol_version":1}}'
        )

    monkeypatch.setattr("fresnica.rust_core_client.subprocess.run", fake_run)

    assert client.version()["core_version"] == "test"
    assert client.binary == binary.resolve()
    assert calls[0][0] == [str(binary.resolve())]


def test_execute_error_reports_resolved_binary_and_os_reason(tmp_path, monkeypatch):
    binary = tmp_path / "fresnica-core"
    _make_executable(binary)
    client = RustCoreClient(binary)

    def fail_run(*args, **kwargs):
        raise OSError(8, "Exec format error")

    monkeypatch.setattr("fresnica.rust_core_client.subprocess.run", fail_run)

    with pytest.raises(RustCoreUnavailableError) as exc_info:
        client.version()

    message = str(exc_info.value)
    assert str(binary.resolve()) in message
    assert "Exec format error" in message
