"""Launch Fresnica TUI."""


def run(runtime):
    from ...tui.system_app import FresnicaApp

    return FresnicaApp(runtime).run()
