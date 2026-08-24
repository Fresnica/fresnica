"""Launch Fresnica TUI."""


def core_subtitle(runtime) -> str:
    if getattr(runtime, "core_client", None) is not None:
        return "Stellar Wallet · Rust Core"
    return "Stellar Wallet · Python Reference"


def run(runtime):
    from ...tui.system_app import FresnicaApp

    app = FresnicaApp(runtime)
    app.sub_title = core_subtitle(runtime)
    return app.run()
