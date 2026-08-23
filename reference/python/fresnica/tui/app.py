"""Product-facing Fresnica TUI shell.

The stable transaction and wallet workflows live in app_base. This layer owns
presentation preferences, transaction-level activity, wallet-management
information architecture, and pair-scoped SDEX presentation.
"""

from decimal import Decimal, InvalidOperation

from rich.text import Text
from textual import work
from textual.binding import Binding

from ..errors import (
    FresnicaError,
    NetworkError,
    TransactionPendingError,
    TransactionSubmissionUncertain,
    TrustlineConfirmationRequired,
    WalletLockedError,
    WalletNotFoundError,
)
from ..history_service import is_suspicious_claimable_activity
from ..manager import WalletState
from ..models import OfferIntent
from ..presentation import format_timestamp, offer_outcome_summary
from ..trustline_policy import FRESNICA_TRUSTLINE_LIMIT_TEXT
from .activity_presentation import activity_display_summary, activity_metadata, activity_text
from .app_base import FresnicaApp as BaseFresnicaApp
from .asset_details import AssetDetailAction, AssetDetailsScreen, PrefilledSendDialog
from .dex import DexOfferAction, DexScreen, MarketPairDialog
from .review_dialog import ReviewPresentationDialog
from .screens import ConfirmDialog
from .trustlines import TrustlineAction, TrustlineScreen
from .wallet_management import WalletManagerDialog


class FresnicaApp(BaseFresnicaApp):
    BINDINGS = [
        *BaseFresnicaApp.BINDINGS,
        Binding("d", "dex", "DEX"),
        Binding("t", "trustlines", "Manage Assets"),
    ]

    def __init__(self, runtime):
        super().__init__(runtime)
        settings = getattr(runtime, "settings", None)
        self._show_zero_balances = bool(
            getattr(settings, "show_zero_balances", False)
        )
        self._send_asset_override: str | None = None
        self._pending_dex_unlock = None
        self._pending_dex_submit = None
        self._pending_trustline_unlock = None
        self._pending_trustline_submit = None

    def on_mount(self) -> None:
        settings = getattr(self.runtime, "settings", None)
        stored_theme = getattr(settings, "theme", None)
        if stored_theme and stored_theme in self.available_themes:
            self.theme = stored_theme
        self.watch(self, "theme", self._persist_theme, init=False)
        # Textual dispatches Mount handlers through the class MRO, so the base
        # on_mount is invoked automatically after this handler. Calling super()
        # here initialized DataTable columns twice. Schedule cached presentation
        # after the complete Mount dispatch instead.
        self.call_later(self._apply_initial_cached_wallet)

    def _apply_initial_cached_wallet(self) -> None:
        try:
            record = self.runtime.wallet_manager.get_record()
        except WalletNotFoundError:
            return
        self._apply_cached_wallet(record.name)

    def _persist_theme(self, old_theme: str, new_theme: str) -> None:
        if old_theme == new_theme:
            return
        settings = getattr(self.runtime, "settings", None)
        store = getattr(self.runtime, "settings_store", None)
        if settings is None or store is None:
            return
        settings.theme = new_theme
        store.save(settings)

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

    def action_send(self) -> None:
        if self._send_asset_override is None:
            self._send_asset_override = self._selected_balance_asset()
        try:
            record = self.runtime.wallet_manager.get_record()
        except WalletNotFoundError:
            self._send_asset_override = None
            super().action_send()
            return
        if not record.watch_only and not self._ensure_write_clear(record):
            self._send_asset_override = None
            return
        super().action_send()

    def _open_send(self) -> None:
        asset = self._send_asset_override
        self._send_asset_override = None
        if asset is None:
            super()._open_send()
            return
        record = self.runtime.wallet_manager.get_record()
        self.push_screen(
            PrefilledSendDialog(record.name, asset),
            self._on_send_request,
        )

    def on_data_table_row_selected(self, event) -> None:
        if getattr(event.data_table, "id", None) != "balances":
            return
        balance = self._selected_balance_view()
        if balance is not None:
            self.push_screen(AssetDetailsScreen(balance), self._on_asset_detail_action)

    def _on_asset_detail_action(self, action: AssetDetailAction | None) -> None:
        if action is None:
            return
        if action.kind == "send":
            self._send_asset_override = action.asset
            self.action_send()
            return
        if action.kind == "receive":
            try:
                record = self.runtime.wallet_manager.get_record()
            except WalletNotFoundError:
                return
            self._show_notice(
                f"Receive {action.asset.split(':', 1)[0]}",
                f"Address:\n{record.address}\n\nAsset:\n{action.asset}",
            )
            return
        if action.kind == "trustline":
            self.action_trustlines()

    def _visible_balance_views(self):
        items = list(self._last_balances)
        if not self._show_zero_balances:
            items = [
                item
                for item in items
                if not (
                    item.balance == 0
                    and item.selling_liabilities == 0
                    and item.buying_liabilities == 0
                )
            ]
        return items

    def _selected_balance_view(self):
        items = self._visible_balance_views()
        if not items:
            return None
        table = self.query_one("#balances")
        row = max(0, min(table.cursor_row, len(items) - 1))
        return items[row]

    def _selected_balance_asset(self) -> str | None:
        if getattr(self.focused, "id", None) != "balances":
            return None
        balance = self._selected_balance_view()
        if balance is None or balance.asset.is_liquidity_pool:
            return None
        if balance.asset.is_native:
            return "XLM"
        return f"{balance.asset.code}:{balance.asset.issuer}"

    def _render_history(self) -> None:
        table = self.query_one("#history")
        table.clear()
        settings = getattr(self.runtime, "settings", None)
        use_local = bool(getattr(settings, "use_local_time", True))
        hide_suspicious = bool(
            getattr(settings, "hide_suspicious_claimables", False)
        )
        try:
            session = self.runtime.wallet_manager.view()
            account = session.wallet.address()
            contacts, domains = activity_metadata(self.runtime, session.wallet)
        except (FresnicaError, ValueError):
            account = getattr(self._last_record, "address", "") if self._last_record else ""
            contacts, domains = {}, {}
        for item in self._last_history:
            if (
                hide_suspicious
                and hasattr(item, "operations")
                and is_suspicious_claimable_activity(item)
            ):
                continue
            summary = activity_display_summary(
                item,
                account,
                contacts,
                domains,
            )
            table.add_row(
                format_timestamp(item.created_at, local=use_local),
                activity_text(item, summary, account),
            )
        if table.columns:
            first_column = next(iter(table.columns.values()))
            first_column.label = Text("Time (local)" if use_local else "Time (UTC)")
            table.refresh()

    def _show_review(self, wallet, services, prepared, network: str) -> None:
        self._pending_send = (wallet, services, prepared, network)
        self._set_status("Transaction ready for review")
        self.push_screen(ReviewPresentationDialog(prepared.review), self._on_review)

    def action_dex(self) -> None:
        try:
            self.runtime.wallet_manager.get_record()
        except WalletNotFoundError:
            self._show_notice("No wallet", "Add or import a wallet before opening the DEX.")
            return
        self.push_screen(MarketPairDialog(), self._open_dex_market)

    def _open_dex_market(self, pair) -> None:
        if pair is None:
            return
        self.push_screen(DexScreen(self.runtime, pair, self._on_dex_action))

    def _on_dex_action(self, screen: DexScreen, action: DexOfferAction) -> None:
        manager = self.runtime.wallet_manager
        try:
            record = manager.get_record()
            state = manager.state(record.name)
        except WalletNotFoundError:
            self._show_notice("No wallet", "Add or import a wallet before trading.")
            return

        if state is WalletState.WATCH_ONLY:
            screen.set_status("Watch-only wallet · market data is available, signing is not.")
            self._show_notice(
                "Watch-only wallet",
                "This wallet can inspect the market, offers, and fills but cannot sign DEX operations.",
            )
            return

        if not self._ensure_write_clear(record, screen=screen):
            return

        if state is WalletState.LOCKED:
            self._pending_dex_unlock = (screen, action)
            self._request_unlock(record.name, after="dex")
            return

        self._prepare_dex_action(screen, action)

    def action_trustlines(self) -> None:
        try:
            self.runtime.wallet_manager.get_record()
        except WalletNotFoundError:
            self._show_notice("No wallet", "Add or import a wallet before managing assets.")
            return
        self.push_screen(TrustlineScreen(self.runtime, self._on_trustline_action))

    def _on_trustline_action(
        self,
        screen: TrustlineScreen,
        action: TrustlineAction,
    ) -> None:
        manager = self.runtime.wallet_manager
        try:
            record = manager.get_record()
            state = manager.state(record.name)
        except WalletNotFoundError:
            self._show_notice("No wallet", "Add or import a wallet before changing assets.")
            return

        if state is WalletState.WATCH_ONLY:
            screen.set_status("Watch-only wallet · issued assets are read-only.")
            self._show_notice(
                "Watch-only wallet",
                "This wallet can inspect issued assets but cannot sign ChangeTrust operations.",
            )
            return

        if not self._ensure_write_clear(record, screen=screen):
            return

        if state is WalletState.LOCKED:
            self._pending_trustline_unlock = (screen, action)
            self._request_unlock(record.name, after="trustline")
            return

        self._prepare_trustline_action(screen, action)

    def _ensure_write_clear(
        self,
        record,
        screen: DexScreen | TrustlineScreen | None = None,
    ) -> bool:
        services = self.runtime.services_for(record.network)
        pending = getattr(services, "pending_transaction_service", None)
        if pending is None:
            return True
        try:
            resolutions = pending.ensure_clear(record.address)
        except TransactionPendingError as exc:
            message = (
                "A previous submission still has unknown status. "
                f"Fresnica will not create another transaction for this account until "
                f"Horizon resolves {exc.tx_hash}."
            )
            if screen is not None and screen.is_mounted:
                screen.set_status(f"Pending transaction · {exc.tx_hash}")
            else:
                self._set_status(f"Pending transaction · {exc.tx_hash}")
            self._show_notice("Transaction pending", message)
            return False
        except NetworkError as exc:
            self._show_error(exc)
            return False

        recovered = [item for item in resolutions if item.status == "confirmed"]
        expired = [item for item in resolutions if item.status == "expired"]
        if recovered:
            message = _pending_resolution_message(recovered)
            if screen is not None and screen.is_mounted:
                screen.set_status(message)
            else:
                self._set_status(message)
        elif expired:
            message = _pending_resolution_message(expired)
            if screen is not None and screen.is_mounted:
                screen.set_status(message)
            else:
                self._set_status(message)
        return True

    def _on_unlock_response(
        self,
        wallet_name: str,
        after: str | None,
        password: str | None,
    ) -> None:
        if after not in {"dex", "trustline"}:
            if after == "send" and password is None:
                self._send_asset_override = None
            super()._on_unlock_response(wallet_name, after, password)
            return
        if password is None:
            if after == "dex":
                self._pending_dex_unlock = None
            else:
                self._pending_trustline_unlock = None
            return

        manager = self.runtime.wallet_manager
        try:
            manager.set_default(wallet_name)
            manager.unlock(wallet_name, password)
        except (FresnicaError, ValueError) as exc:
            self._request_unlock(wallet_name, after=after, error=str(exc))
            return

        self.refresh_wallet(f"Unlocked wallet {wallet_name}")
        if after == "dex":
            pending = self._pending_dex_unlock
            self._pending_dex_unlock = None
            if pending is not None:
                screen, action = pending
                if screen.is_mounted:
                    self._prepare_dex_action(screen, action)
            return

        pending = self._pending_trustline_unlock
        self._pending_trustline_unlock = None
        if pending is not None:
            screen, action = pending
            if screen.is_mounted:
                self._prepare_trustline_action(screen, action)

    @work(thread=True, exit_on_error=False)
    def _prepare_dex_action(
        self,
        screen: DexScreen,
        action: DexOfferAction,
        allow_trustline: bool = False,
    ) -> None:
        try:
            manager = self.runtime.wallet_manager
            record = manager.get_record()
            session = manager.current()
            if session is None or session.record.name != record.name:
                raise WalletLockedError(f'Wallet "{record.name}" is locked')
            services = self.runtime.services_for(record.network)
            offer_service = services.offer_service

            if action.kind == "cancel":
                if action.offer is None:
                    raise ValueError("No offer selected for cancellation")
                prepared = offer_service.prepare_cancel(
                    record.name,
                    session.wallet,
                    action.offer,
                    pair=action.pair,
                )
            else:
                if action.side not in ("buy", "sell"):
                    raise ValueError("DEX action is missing BUY/SELL side")
                try:
                    amount = Decimal(str(action.amount))
                    price = Decimal(str(action.price))
                except InvalidOperation as exc:
                    raise ValueError("Invalid DEX amount or price") from exc
                intent = OfferIntent(
                    pair=action.pair,
                    side=action.side,
                    amount=amount,
                    price=price,
                )
                if action.kind == "create":
                    prepared = offer_service.prepare_create(
                        record.name,
                        session.wallet,
                        intent,
                        allow_trustline=allow_trustline,
                    )
                elif action.kind == "update":
                    if action.offer is None:
                        raise ValueError("No offer selected for update")
                    prepared = offer_service.prepare_update(
                        record.name,
                        session.wallet,
                        action.offer,
                        intent,
                    )
                else:
                    raise ValueError(f"Unsupported DEX action: {action.kind}")

            self.call_from_thread(
                self._show_dex_review,
                screen,
                session.wallet,
                services,
                prepared,
                record.network,
            )
        except TrustlineConfirmationRequired:
            self.call_from_thread(self._confirm_dex_trustline, screen, action)
        except (FresnicaError, ValueError) as exc:
            self.call_from_thread(self._finish_dex_prepare_error, screen, exc)

    @work(thread=True, exit_on_error=False)
    def _prepare_trustline_action(
        self,
        screen: TrustlineScreen,
        action: TrustlineAction,
    ) -> None:
        try:
            manager = self.runtime.wallet_manager
            record = manager.get_record()
            session = manager.current()
            if session is None or session.record.name != record.name:
                raise WalletLockedError(f'Wallet "{record.name}" is locked')
            services = self.runtime.services_for(record.network)
            service = services.trustline_service
            if action.kind == "add":
                prepared = service.prepare_add(
                    record.name,
                    session.wallet,
                    action.asset,
                    limit=action.limit,
                )
            elif action.kind == "limit":
                prepared = service.prepare_limit(
                    record.name,
                    session.wallet,
                    action.asset,
                    action.limit,
                )
            elif action.kind == "remove":
                prepared = service.prepare_remove(
                    record.name,
                    session.wallet,
                    action.asset,
                )
            else:
                raise ValueError(f"Unsupported trustline action: {action.kind}")
            self.call_from_thread(
                self._show_trustline_review,
                screen,
                session.wallet,
                services,
                prepared,
                record.network,
            )
        except (FresnicaError, ValueError) as exc:
            self.call_from_thread(self._finish_trustline_prepare_error, screen, exc)

    def _confirm_dex_trustline(
        self,
        screen: DexScreen,
        action: DexOfferAction,
    ) -> None:
        if not screen.is_mounted:
            return
        receiving = action.pair.base if action.side == "buy" else action.pair.counter
        identity = (
            "XLM"
            if receiving.is_native
            else f"{receiving.code}:{receiving.issuer}"
        )
        screen.set_status(f"Receiving {receiving.display} needs a trustline confirmation.")
        self.push_screen(
            ConfirmDialog(
                "Create receiving trustline",
                f"This offer requires a new trustline for {identity}. "
                f"Fresnica will use trustline limit {FRESNICA_TRUSTLINE_LIMIT_TEXT} "
                "and submit ChangeTrust with the offer in one transaction.",
                "Create & continue",
            ),
            lambda confirmed: self._on_dex_trustline_confirmation(
                screen,
                action,
                confirmed,
            ),
        )

    def _on_dex_trustline_confirmation(
        self,
        screen: DexScreen,
        action: DexOfferAction,
        confirmed: bool,
    ) -> None:
        if not confirmed:
            screen.set_status("Offer cancelled · receiving trustline was not approved.")
            return
        screen.set_status("Preparing offer with confirmed receiving trustline...")
        self._prepare_dex_action(screen, action, allow_trustline=True)

    def _finish_dex_prepare_error(self, screen: DexScreen, error) -> None:
        if screen.is_mounted:
            screen.set_status(f"Offer preparation failed: {error}")
        self._show_error(error)

    def _finish_trustline_prepare_error(self, screen: TrustlineScreen, error) -> None:
        if screen.is_mounted:
            screen.set_status(f"Trustline preparation failed: {error}")
        self._show_error(error)

    def _show_dex_review(
        self,
        screen: DexScreen,
        wallet,
        services,
        prepared,
        network: str,
    ) -> None:
        if not screen.is_mounted:
            return
        self._pending_dex_submit = (screen, wallet, services, prepared, network)
        screen.set_status("DEX operation ready for review.")
        self.push_screen(ReviewPresentationDialog(prepared.review), self._on_dex_review)

    def _show_trustline_review(
        self,
        screen: TrustlineScreen,
        wallet,
        services,
        prepared,
        network: str,
    ) -> None:
        if not screen.is_mounted:
            return
        self._pending_trustline_submit = (screen, wallet, services, prepared, network)
        screen.set_status("Trustline operation ready for review.")
        self.push_screen(
            ReviewPresentationDialog(prepared.review),
            self._on_trustline_review,
        )

    def _on_dex_review(self, confirmed: bool) -> None:
        pending = self._pending_dex_submit
        if pending is None:
            return
        screen = pending[0]
        if not confirmed:
            self._pending_dex_submit = None
            if screen.is_mounted:
                screen.set_status("DEX operation cancelled · wallet remains unlocked.")
            return
        if screen.is_mounted:
            screen.set_status("Submitting DEX operation...")
        self._submit_dex_pending()

    def _on_trustline_review(self, confirmed: bool) -> None:
        pending = self._pending_trustline_submit
        if pending is None:
            return
        screen = pending[0]
        if not confirmed:
            self._pending_trustline_submit = None
            if screen.is_mounted:
                screen.set_status("Trustline operation cancelled · wallet remains unlocked.")
            return
        if screen.is_mounted:
            screen.set_status("Submitting trustline operation...")
        self._submit_trustline_pending()

    @work(thread=True, exit_on_error=False)
    def _submit_dex_pending(self) -> None:
        pending = self._pending_dex_submit
        if pending is None:
            return
        screen, wallet, services, prepared, network = pending
        try:
            services.offer_service.sign(wallet, prepared)
            result = services.offer_service.submit(prepared)
            self.call_from_thread(self._finish_dex_submit, screen, result, network, None)
        except (FresnicaError, ValueError) as exc:
            self.call_from_thread(self._finish_dex_submit, screen, None, network, exc)
        finally:
            self._pending_dex_submit = None

    @work(thread=True, exit_on_error=False)
    def _submit_trustline_pending(self) -> None:
        pending = self._pending_trustline_submit
        if pending is None:
            return
        screen, wallet, services, prepared, network = pending
        try:
            services.trustline_service.sign(wallet, prepared)
            result = services.trustline_service.submit(prepared)
            self.call_from_thread(
                self._finish_trustline_submit,
                screen,
                result,
                network,
                None,
            )
        except (FresnicaError, ValueError) as exc:
            self.call_from_thread(
                self._finish_trustline_submit,
                screen,
                None,
                network,
                exc,
            )
        finally:
            self._pending_trustline_submit = None

    def _finish_dex_submit(self, screen: DexScreen, result, network: str, error) -> None:
        if isinstance(error, TransactionSubmissionUncertain):
            message = f"Submission status unknown · {error.tx_hash}"
            if screen.is_mounted:
                screen.set_status(message)
            self._show_notice(
                "Submission status unknown",
                f"The transaction may already be on Stellar. Fresnica saved {error.tx_hash} "
                "and will resolve it by hash before allowing another write from this account.",
            )
            return
        if error is not None:
            if screen.is_mounted:
                screen.set_status(f"DEX submission failed: {error}")
            self._show_error(error)
            return

        message = (
            f"DEX transaction submitted on {network}: {result.hash}"
            + (f" · ledger {result.ledger}" if result.ledger is not None else "")
        )
        outcome = offer_outcome_summary(getattr(result, "offer_outcome", None))
        if outcome:
            message += f" · {outcome}"
        if screen.is_mounted:
            screen.set_status(message)
            screen.refresh_market()
        self.refresh_wallet(message)

    def _finish_trustline_submit(
        self,
        screen: TrustlineScreen,
        result,
        network: str,
        error,
    ) -> None:
        if isinstance(error, TransactionSubmissionUncertain):
            message = f"Submission status unknown · {error.tx_hash}"
            if screen.is_mounted:
                screen.set_status(message)
            self._show_notice(
                "Submission status unknown",
                f"The transaction may already be on Stellar. Fresnica saved {error.tx_hash} "
                "and will resolve it by hash before allowing another write from this account.",
            )
            return
        if error is not None:
            if screen.is_mounted:
                screen.set_status(f"Trustline submission failed: {error}")
            self._show_error(error)
            return

        message = (
            f"Trustline transaction submitted on {network}: {result.hash}"
            + (f" · ledger {result.ledger}" if result.ledger is not None else "")
        )
        if screen.is_mounted:
            screen.set_status(message)
            screen.refresh_trustlines()
        self.refresh_wallet(message)

    def _finish_send(self, result, network: str, error) -> None:
        if isinstance(error, TransactionSubmissionUncertain):
            self._set_status(f"Submission status unknown · {error.tx_hash}")
            self._show_notice(
                "Submission status unknown",
                f"The transaction may already be on Stellar. Fresnica saved {error.tx_hash} "
                "and will resolve it by hash before allowing another write from this account.",
            )
            return
        super()._finish_send(result, network, error)

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
                self._set_sync("No cached data yet · loading account from Horizon...")
        except (FresnicaError, ValueError):
            # Cache presentation is opportunistic. The background refresh remains
            # authoritative and will surface a real network/protocol error.
            return

    def _set_refresh_stage(self, selected_name: str | None, message: str) -> None:
        if self._is_current_wallet(selected_name):
            self._set_sync(message)

    @work(thread=True, exit_on_error=False)
    def fund_wallet(self, wallet_name: str) -> None:
        """Verify an unknown Testnet account only after the user requests funding."""
        try:
            record = self.runtime.wallet_manager.get_record(wallet_name)
            if record.network != "testnet":
                raise NetworkError("Friendbot is only available on testnet")
            services = self.runtime.services_for("testnet")
            if services.testnet_service is None:
                raise NetworkError("Friendbot is unavailable for testnet")

            adapter = getattr(services.balance_service, "adapter", None)
            checker = getattr(adapter, "account_exists", None)
            if checker is not None and checker(record.address):
                self.call_from_thread(
                    self.refresh_wallet,
                    f'Wallet "{record.name}" already exists on testnet; Friendbot not needed',
                )
                return

            result = services.testnet_service.fund(record.address)
            tx_hash = result.get("hash") if isinstance(result, dict) else None
            message = f'Funded wallet "{record.name}" on testnet'
            if tx_hash:
                message += f"; transaction {tx_hash}"
            self.call_from_thread(self.refresh_wallet, message)
        except (FresnicaError, ValueError) as exc:
            self.call_from_thread(self._show_error, exc)

    @work(exclusive=True, thread=True, exit_on_error=False)
    def _refresh_wallet(self, ready_message: str | None = None) -> None:
        record = None
        try:
            session = self.runtime.wallet_manager.view()
            record = session.record
            selected_name = record.name
            services = self.runtime.services_for(record.network)
            pending = getattr(services, "pending_transaction_service", None)
            if pending is not None:
                self.call_from_thread(
                    self._set_refresh_stage,
                    selected_name,
                    "Checking pending transaction state...",
                )
                pending_message = _pending_resolution_message(
                    pending.resolve(record.address)
                )
                if pending_message:
                    ready_message = (
                        f"{ready_message} · {pending_message}"
                        if ready_message
                        else pending_message
                    )
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
                    self.call_from_thread(
                        self._set_refresh_stage,
                        selected_name,
                        "Checking Testnet account...",
                    )
                    exists = bool(exists_checker(record.address))
                    if not exists:
                        self.call_from_thread(
                            self._apply_unfunded_testnet,
                            selected_name,
                            record,
                            ready_message,
                        )
                        return

            self.call_from_thread(
                self._set_refresh_stage,
                selected_name,
                "Loading balances, issuer metadata, and liquidity...",
            )
            balances, positions = balance_service.get_portfolio_views(session.wallet)
            history_service = services.history_service
            activity_getter = getattr(
                history_service,
                "get_activity_views",
                history_service.get_views,
            )
            self.call_from_thread(
                self._set_refresh_stage,
                selected_name,
                "Loading recent activity...",
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


def _pending_resolution_message(resolutions) -> str | None:
    if not resolutions:
        return None
    pending = [item for item in resolutions if item.status == "pending"]
    if pending:
        return f"Transaction pending · {pending[0].pending.tx_hash}"
    confirmed = [item for item in resolutions if item.status == "confirmed"]
    if confirmed:
        item = confirmed[-1]
        successful = bool((item.transaction or {}).get("successful", True))
        state = "confirmed" if successful else "finalized as failed"
        return f"Recovered transaction {state} · {item.pending.tx_hash}"
    expired = [item for item in resolutions if item.status == "expired"]
    if expired:
        return f"Uncertain transaction not found after timeout · {expired[-1].pending.tx_hash}"
    return None


def run_tui(runtime):
    return FresnicaApp(runtime).run()
