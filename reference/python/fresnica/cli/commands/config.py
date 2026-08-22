"""Network configuration command helpers."""


def show(runtime):
    return {
        "home": str(runtime.home),
        "default_network": "mainnet",
    }
