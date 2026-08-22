"""Fresnica Textual application.

The TUI consumes the same services as CLI.
"""


class FresnicaApp:
    def __init__(self, context):
        self.context = context

    def run(self):
        # Textual implementation will replace this placeholder.
        return self.context


def run_tui(context=None):
    return FresnicaApp(context).run()
