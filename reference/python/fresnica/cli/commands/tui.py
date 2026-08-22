"""Launch Fresnica TUI."""


def run(runtime):
    from ...tui.app import FresnicaApp

    return FresnicaApp(runtime).run()
