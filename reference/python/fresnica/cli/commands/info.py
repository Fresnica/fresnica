"""Wallet information command."""


def execute_info(runtime, args, renderer):
    record = runtime.wallet_manager.get_record(args.wallet)
    renderer.render_info(record)
    return record
