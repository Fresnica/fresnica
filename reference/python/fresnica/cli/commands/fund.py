"""Fund a Stellar testnet account."""


def run(runtime, address):
    service = runtime.services_for("testnet")
    return service.testnet_service.fund(address)
