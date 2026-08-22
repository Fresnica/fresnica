"""Fresnica Textual application.

The TUI consumes the same services as CLI.
"""


class FresnicaApp:
    def __init__(self, context):
        self.context = context

    def run(self):
        raise NotImplementedError("Textual UI implementation")
