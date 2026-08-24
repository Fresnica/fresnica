"""Product TUI with client-owned system-unlock orchestration."""

from textual import work

from ..client_system_unlock import SystemUnlockError
from ..errors import FresnicaError, InvalidUnlockKeyError
from .app import FresnicaApp as ProductFresnicaApp
from .system_unlock import SystemUnlockEnrollmentDialog
from .wallet_management import WalletManagerDialog


class FresnicaApp(ProductFresnicaApp):
    """Adds system authorization without introducing OS logic into Core/reference crypto."""

    def _open_wallet_manager(self) -> None:
        manager = self.runtime.wallet_manager
        records = manager.list_wallets()
        if not records:
            self._open_add_wallet()
            return

        states = {record.name: manager.state(record.name) for record in records}
        account_exists: dict[str, bool | None] = {}
        for record in records:
            if record.network != "testnet":
                continue
            services = self.runtime.services_for(record.network)
            checker = getattr(services.balance_service, "has_cached_account", None)
            cached = bool(checker(manager.view(record.name).wallet)) if checker else False
            account_exists[record.name] = True if cached else None

        controller = self.runtime.system_unlock
        system_unlock = None
        if controller.available():
            try:
                system_unlock = {
                    record.name: controller.enrolled(record)
                    for record in records
                    if not record.watch_only
                }
            except (SystemUnlockError, FresnicaError, ValueError):
                system_unlock = None

        dialog = WalletManagerDialog(
            records,
            manager.storage.get_default(),
            states,
            account_exists=account_exists,
            system_unlock=system_unlock,
            on_select=self._select_wallet_from_manager,
        )
        self.push_screen(dialog, self._on_wallet_action)

    def _on_wallet_action(self, action) -> None:
        if action is not None and action.wallet_name:
            if action.action == "enable-system-unlock":
                self._request_system_unlock_enrollment(action.wallet_name)
                return
            if action.action == "disable-system-unlock":
                self._disable_system_unlock(action.wallet_name)
                return
        super()._on_wallet_action(action)

    def _request_unlock(
        self,
        wallet_name: str,
        after: str | None = None,
        error: str | None = None,
    ) -> None:
        if error is None:
            try:
                record = self.runtime.wallet_manager.get_record(wallet_name)
                if self.runtime.system_unlock.enrolled(record):
                    self._set_status(
                        f"Requesting {self.runtime.system_unlock_backend.label} for {wallet_name}..."
                    )
                    self._system_unlock_wallet(wallet_name, after)
                    return
            except (SystemUnlockError, FresnicaError, ValueError):
                pass
        self._password_unlock(wallet_name, after, error)

    def _password_unlock(
        self,
        wallet_name: str,
        after: str | None,
        error: str | None = None,
    ) -> None:
        ProductFresnicaApp._request_unlock(self, wallet_name, after, error)

    @work(thread=True, exit_on_error=False)
    def _system_unlock_wallet(self, wallet_name: str, after: str | None) -> None:
        manager = self.runtime.wallet_manager
        try:
            manager.set_default(wallet_name)
            self.runtime.system_unlock.unlock(manager, wallet_name)
        except InvalidUnlockKeyError:
            try:
                self.runtime.system_unlock.disable(manager, wallet_name)
            except (SystemUnlockError, FresnicaError, ValueError):
                pass
            self.call_from_thread(
                self._password_unlock,
                wallet_name,
                after,
                "Stored system unlock key is stale. Enter the app passcode to continue.",
            )
            return
        except (SystemUnlockError, FresnicaError, ValueError):
            self.call_from_thread(
                self._password_unlock,
                wallet_name,
                after,
                "System authentication was unavailable. Enter the app passcode to continue.",
            )
            return

        self.call_from_thread(self._finish_system_unlock, wallet_name, after)

    def _finish_system_unlock(self, wallet_name: str, after: str | None) -> None:
        self.refresh_wallet(f"Unlocked wallet {wallet_name} with system authentication")
        if after == "send":
            self._open_send()
            return
        if after == "dex":
            pending = self._pending_dex_unlock
            self._pending_dex_unlock = None
            if pending is not None:
                screen, action = pending
                if screen.is_mounted:
                    self._prepare_dex_action(screen, action)
            return
        if after == "trustline":
            pending = self._pending_trustline_unlock
            self._pending_trustline_unlock = None
            if pending is not None:
                screen, action = pending
                if screen.is_mounted:
                    self._prepare_trustline_action(screen, action)

    def _request_system_unlock_enrollment(
        self,
        wallet_name: str,
        error: str | None = None,
    ) -> None:
        backend = self.runtime.system_unlock_backend
        if not backend.available():
            self._show_notice(
                "System unlock unavailable",
                "This TUI client has no operating-system system-unlock backend configured.",
            )
            return
        self.push_screen(
            SystemUnlockEnrollmentDialog(wallet_name, backend.label, error),
            lambda passcode: self._on_system_unlock_enrollment(
                wallet_name,
                passcode,
            ),
        )

    def _on_system_unlock_enrollment(
        self,
        wallet_name: str,
        passcode: str | None,
    ) -> None:
        if passcode is None:
            return
        self._set_status(f"Enrolling system unlock for {wallet_name}...")
        self._enroll_system_unlock(wallet_name, passcode)

    @work(thread=True, exit_on_error=False)
    def _enroll_system_unlock(self, wallet_name: str, passcode: str) -> None:
        try:
            self.runtime.system_unlock.enroll(
                self.runtime.wallet_manager,
                wallet_name,
                passcode,
            )
        except (SystemUnlockError, FresnicaError, ValueError) as exc:
            self.call_from_thread(
                self._request_system_unlock_enrollment,
                wallet_name,
                str(exc),
            )
            return
        self.call_from_thread(
            self.refresh_wallet,
            f"System unlock enabled for {wallet_name}",
        )

    def _disable_system_unlock(self, wallet_name: str) -> None:
        try:
            self.runtime.system_unlock.disable(
                self.runtime.wallet_manager,
                wallet_name,
            )
        except (SystemUnlockError, FresnicaError, ValueError) as exc:
            self._show_error(exc)
            return
        self.refresh_wallet(f"System unlock disabled for {wallet_name}")

    def _delete_wallet(self, name: str, confirmed: bool) -> None:
        if confirmed:
            try:
                record = self.runtime.wallet_manager.get_record(name)
                if self.runtime.system_unlock.enrolled(record):
                    self.runtime.system_unlock.disable(
                        self.runtime.wallet_manager,
                        name,
                    )
            except (SystemUnlockError, FresnicaError, ValueError) as exc:
                self._show_error(exc)
                return
        super()._delete_wallet(name, confirmed)


def run_tui(runtime):
    return FresnicaApp(runtime).run()
