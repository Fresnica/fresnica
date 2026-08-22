"""Product-facing Fresnica TUI shell.

The stable transaction and wallet workflows live in app_base. This layer owns
presentation preferences, transaction-level activity, and wallet-management
information architecture.
"""

from textual import work

from ..errors import FresnicaError, WalletNotFoundError
from .app_base import FresnicaApp as BaseFresnicaApp
from .wallet_management import WalletManagerDialog


class FresnicaApp(BaseFresnicaApp):
    def __init__(self, runtime):
        super().__init__(runtime)
        settings = getattr(runtime, "settings", None)
        self._show_zero_balances = bool(
            getattr(settings, "show_zero_balances", False)
        )

    def action_toggle_zero(self) -> None:
        self._show_zero_balances = not self._show_zero_balances
        settings = getattr(self.runtime, "settings", None)
        store = getattr(self.runtime, "settings_store", None)
        if settings is not None:
            settings.show_zero_balances = self._show_zero_balances
            if store is not None:
                store.save(settings)
        self._render_balances()
        mode = "shown" if self._show_zero_balances else "hidden"
        self._set_sync(f"Zero-balance assets {mode} · preference saved")

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

        dialog = WalletManagerDialog(
            records,
            manager.storage.get_default(),
            states,
            account_exists=account_exists,
            on_select=self._select_wallet_from_manager,
        )
        self.push_screen(dialog, self._on_wallet_action)

    def _select_wallet_from_manager(self, wallet_name: str):
        """Change wallet identity only; dashboard I/O waits until the dialog closes."""
        manager = self.runtime.wallet_manager
        manager.set_default(wallet_name)
        return {
            record.name: manager.state(record.name)
            for record in manager.list_wallets()
        }

    def _on_wallet_action(self, action) -> None:
        if action is not None and action.action == "use" and action.wallet_name:
            manager = self.runtime.wallet_manager
            manager.set_default(action.wallet_name)
            self._apply_cached_wallet(action.wallet_name)
            self._refresh_wallet(f"Selected wallet {action.wallet_name}")
            return
        super()._on_wallet_action(action)

    def _apply_cached_wallet(self, wallet_name: str) -> None:
        """Render locally cached portfolio/activity before any Horizon request."""
        manager = self.runtime.wallet_manager
        try:
            session = manager.view(wallet_name)
            services = self.runtime.services_for(session.record.network)
            cached_portfolio = getattr(
                services.balance_service,
                "get_cached_portfolio_views",
                None,
            )
            if cached_portfolio is not None:
                balances, positions = cached_portfolio(session.wallet)
            else:
                balances, positions = [], []

            history_service = services.history_service
            activity_getter = getattr(
                history_service,
                "get_activity_views",
                history_service.get_views,
            )
            history = activity_getter(session.wallet, limit=20, refresh=False)
            self._apply_wallet(
                session.record,
                balances,
                positions,
                history,
                None,
                None,
            )
            if balances or positions or history:
                self._set_sync("Showing cached data · refreshing in background...")
            else:
                self._set_sync("No cached data yet · refreshing in background...")
        except (FresnicaError, ValueError):
            # Cache presentation is opportunistic. The background refresh remains
            # authoritative and will surface a real network/protocol error.
            return

    @work(exclusive=True, thread=True, exit_on_error=False)
    def _refresh_wallet(self, ready_message: str | None = None) -> None:
        record = None
        try:
            session = self.runtime.wallet_manager.view()
            record = session.record
            selected_name = record.name
            services = self.runtime.services_for(record.network)
            balance_service = services.balance_service

            # A brand-new Testnet wallet is the one case where a full account
            # load would otherwise fail noisily. Probe existence only during the
            # final dashboard refresh, never while navigating Wallet Management.
            cache_checker = getattr(balance_service, "has_cached_account", None)
            has_cached_account = bool(cache_checker(session.wallet)) if cache_checker else False
            if record.network == "testnet" and not has_cached_account:
                adapter = getattr(balance_service, "adapter", None)
                exists_checker = getattr(adapter, "account_exists", None)
                if exists_checker is not None:
                    exists = bool(exists_checker(record.address))
                    if not exists:
                        self.call_from_thread(
                            self._apply_unfunded_testnet,
                            selected_name,
                            record,
                            ready_message,
                        )
                        return

            balances, positions = balance_service.get_portfolio_views(session.wallet)
            history_service = services.history_service
            activity_getter = getattr(
                history_service,
                "get_activity_views",
                history_service.get_views,
            )
            history = activity_getter(session.wallet, limit=20)
            self.call_from_thread(
                self._apply_wallet_if_current,
                selected_name,
                record,
                balances,
                positions,
                history,
                ready_message,
                None,
            )
        except WalletNotFoundError:
            self.call_from_thread(
                self._apply_wallet_if_current,
                None,
                None,
                [],
                [],
                [],
                ready_message,
                None,
            )
        except (FresnicaError, ValueError) as exc:
            selected_name = record.name if record is not None else None
            self.call_from_thread(
                self._apply_wallet_if_current,
                selected_name,
                record,
                [],
                [],
                [],
                ready_message,
                exc,
            )

    def _apply_unfunded_testnet(self, selected_name, record, ready_message) -> None:
        if not self._is_current_wallet(selected_name):
            return
        self._apply_wallet(record, [], [], [], ready_message, None)
        self._set_sync("Testnet account is not funded")

    def _apply_wallet_if_current(
        self,
        selected_name,
        record,
        balances,
        positions,
        history,
        ready_message,
        error,
    ) -> None:
        # Keep only this final stale-result check. Wallet Management itself no
        # longer starts competing dashboard refreshes while the user navigates.
        if not self._is_current_wallet(selected_name):
            return
        self._apply_wallet(
            record,
            balances,
            positions,
            history,
            ready_message,
            error,
        )

    def _is_current_wallet(self, wallet_name: str | None) -> bool:
        try:
            current = self.runtime.wallet_manager.get_record().name
        except WalletNotFoundError:
            current = None
        return current == wallet_name


def run_tui(runtime):
    return FresnicaApp(runtime).run()
