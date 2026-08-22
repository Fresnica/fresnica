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
        self.push_screen(
            WalletManagerDialog(records, manager.storage.get_default(), states),
            self._on_wallet_action,
        )

    @work(exclusive=True, thread=True, exit_on_error=False)
    def _refresh_wallet(self, ready_message: str | None = None) -> None:
        record = None
        try:
            session = self.runtime.wallet_manager.view()
            record = session.record
            services = self.runtime.services_for(record.network)
            balances, positions = services.balance_service.get_portfolio_views(session.wallet)
            history_service = services.history_service
            activity_getter = getattr(
                history_service,
                "get_activity_views",
                history_service.get_views,
            )
            history = activity_getter(session.wallet, limit=20)
            self.call_from_thread(
                self._apply_wallet,
                record,
                balances,
                positions,
                history,
                ready_message,
                None,
            )
        except WalletNotFoundError:
            self.call_from_thread(self._apply_wallet, None, [], [], [], ready_message, None)
        except (FresnicaError, ValueError) as exc:
            self.call_from_thread(self._apply_wallet, record, [], [], [], ready_message, exc)


def run_tui(runtime):
    return FresnicaApp(runtime).run()
