"""Shared transaction review dialog for the product-facing TUI."""

from textual.app import ComposeResult
from textual.containers import Horizontal, Vertical
from textual.widgets import Button, Label, Static

from ..review_presentation import project_review, review_text
from .screens import ReviewDialog


class ReviewPresentationDialog(ReviewDialog):
    """Render the same UI-neutral review semantics used by command mode."""

    CSS = """
    ReviewPresentationDialog { align: center middle; }
    ReviewPresentationDialog > #dialog { width: 94; height: auto; max-height: 90%; padding: 1 2; border: round $warning; background: $surface; }
    ReviewPresentationDialog #review-text { margin: 1 0; }
    ReviewPresentationDialog #actions { height: auto; align-horizontal: right; }
    ReviewPresentationDialog Button { margin-left: 1; }
    """

    def __init__(self, review):
        super().__init__(review)
        self.presentation = project_review(review)
        self.presentation_text = review_text(review)

    def compose(self) -> ComposeResult:
        with Vertical(id="dialog"):
            yield Label(self.presentation.title)
            yield Static(self.presentation_text, id="review-text")
            with Horizontal(id="actions"):
                yield Button("Cancel", id="cancel")
                yield Button("Confirm", id="confirm", variant="warning")
