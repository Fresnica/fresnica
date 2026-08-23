"""Reusable slash-search overlay for long TUI lists."""

from collections.abc import Callable

from textual.app import ComposeResult
from textual.binding import Binding
from textual.containers import Vertical
from textual.screen import ModalScreen
from textual.widgets import Input, Label


class ListSearchDialog(ModalScreen[str]):
    BINDINGS = [Binding("escape", "clear", "Clear search")]

    CSS = """
    ListSearchDialog { align: center top; padding-top: 3; }
    ListSearchDialog > #search-dialog {
        width: 72;
        height: auto;
        padding: 1 2;
        border: round $accent;
        background: $surface;
    }
    #list-search-input { margin-top: 1; }
    """

    def __init__(
        self,
        initial: str = "",
        *,
        on_change: Callable[[str], None] | None = None,
        label: str = "Search list",
    ):
        super().__init__()
        self.initial = initial
        self.on_change = on_change
        self.label = label

    def compose(self) -> ComposeResult:
        with Vertical(id="search-dialog"):
            yield Label(f"{self.label} · Esc clears")
            yield Input(value=self.initial, placeholder="Type to filter...", id="list-search-input")

    def on_mount(self) -> None:
        field = self.query_one("#list-search-input", Input)
        self.set_focus(field)
        field.cursor_position = len(field.value)

    def on_input_changed(self, event: Input.Changed) -> None:
        if event.input.id == "list-search-input" and self.on_change is not None:
            self.on_change(event.value)

    def on_input_submitted(self, event: Input.Submitted) -> None:
        if event.input.id == "list-search-input":
            self.dismiss(event.value.strip())

    def action_clear(self) -> None:
        if self.on_change is not None:
            self.on_change("")
        self.dismiss("")


def matches_query(query: str, *values) -> bool:
    needle = query.strip().casefold()
    if not needle:
        return True
    return any(needle in str(value or "").casefold() for value in values)
