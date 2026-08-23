"""Local contact-book presentation for the Textual wallet."""

from textual.app import ComposeResult
from textual.binding import Binding
from textual.containers import Horizontal, Vertical
from textual.screen import ModalScreen
from textual.widgets import Button, DataTable, Footer, Input, Label, Static

from ..contacts import Contact, ContactError, ContactStore
from ..presentation import short_address
from .screens import ConfirmDialog


class AddContactDialog(ModalScreen[Contact | None]):
    BINDINGS = [("escape", "cancel", "Cancel")]

    CSS = """
    AddContactDialog { align: center middle; }
    AddContactDialog > #dialog { width: 82; height: auto; padding: 1 2; border: round $accent; background: $surface; }
    AddContactDialog Input { margin-top: 1; }
    AddContactDialog #form-error { color: $error; margin-top: 1; }
    AddContactDialog #actions { height: auto; margin-top: 1; align-horizontal: right; }
    AddContactDialog Button { margin-left: 1; }
    """

    def __init__(self, store: ContactStore, address: str = ""):
        super().__init__()
        self.store = store
        self.address = address

    def compose(self) -> ComposeResult:
        with Vertical(id="dialog"):
            yield Label("Add contact")
            yield Input(placeholder="Name", id="contact-name")
            yield Input(value=self.address, placeholder="Stellar G... address", id="contact-address")
            yield Input(placeholder="Default memo (optional)", id="contact-memo")
            yield Static("", id="form-error")
            with Horizontal(id="actions"):
                yield Button("Cancel", id="cancel")
                yield Button("Add", id="add", variant="primary")

    def action_cancel(self) -> None:
        self.dismiss(None)

    def on_button_pressed(self, event: Button.Pressed) -> None:
        if event.button.id == "cancel":
            self.action_cancel()
            return
        if event.button.id != "add":
            return
        try:
            contact = self.store.add(
                self.query_one("#contact-name", Input).value,
                self.query_one("#contact-address", Input).value,
                self.query_one("#contact-memo", Input).value or None,
            )
        except (ContactError, ValueError) as exc:
            self.query_one("#form-error", Static).update(str(exc))
            return
        self.dismiss(contact)


class ContactBookScreen(ModalScreen[None]):
    BINDINGS = [
        Binding("escape", "close", "Back"),
        Binding("a", "add", "Add"),
        Binding("d", "delete", "Delete"),
    ]

    CSS = """
    ContactBookScreen { layout: vertical; background: $surface; padding: 1 2; }
    #contacts-title { height: 1; text-style: bold; }
    #contacts-status { height: 1; color: $text-muted; margin-bottom: 1; }
    #contacts-table { height: 1fr; }
    """

    def __init__(self, store: ContactStore):
        super().__init__()
        self.store = store
        self._contacts: list[Contact] = []

    def compose(self) -> ComposeResult:
        yield Static("Contacts", id="contacts-title")
        yield Static("Local address book", id="contacts-status")
        yield DataTable(id="contacts-table")
        yield Footer()

    def on_mount(self) -> None:
        table = self.query_one("#contacts-table", DataTable)
        table.add_columns("Name", "Address", "Memo")
        table.cursor_type = "row"
        self.refresh_contacts()

    def action_close(self) -> None:
        self.dismiss(None)

    def action_add(self) -> None:
        self.app.push_screen(AddContactDialog(self.store), self._after_add)

    def action_delete(self) -> None:
        contact = self._selected()
        if contact is None:
            self.query_one("#contacts-status", Static).update("No contact selected")
            return
        self.app.push_screen(
            ConfirmDialog(
                "Delete contact",
                f'Delete contact "{contact.name}" ({short_address(contact.address)})?',
                "Delete",
            ),
            lambda confirmed: self._delete(contact, confirmed),
        )

    def _delete(self, contact: Contact, confirmed: bool) -> None:
        if not confirmed:
            return
        try:
            self.store.remove(contact.name)
        except ContactError as exc:
            self.query_one("#contacts-status", Static).update(str(exc))
            return
        self.refresh_contacts(f'Deleted "{contact.name}"')

    def _after_add(self, contact: Contact | None) -> None:
        if contact is not None:
            self.refresh_contacts(f'Added "{contact.name}"')

    def refresh_contacts(self, message: str | None = None) -> None:
        table = self.query_one("#contacts-table", DataTable)
        table.clear()
        try:
            self._contacts = self.store.list()
        except ContactError as exc:
            self._contacts = []
            self.query_one("#contacts-status", Static).update(str(exc))
            return
        for contact in self._contacts:
            table.add_row(contact.name, short_address(contact.address), contact.memo or "")
        self.query_one("#contacts-status", Static).update(
            message or f"{len(self._contacts)} contacts · stored locally"
        )

    def _selected(self) -> Contact | None:
        if not self._contacts:
            return None
        row = self.query_one("#contacts-table", DataTable).cursor_row
        return self._contacts[max(0, min(row, len(self._contacts) - 1))]
