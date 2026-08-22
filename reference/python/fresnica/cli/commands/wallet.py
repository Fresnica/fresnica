"""Wallet lifecycle commands."""

from getpass import getpass

from ...hdwallet import generate_mnemonic_phrase


def execute_wallet(runtime, args, renderer, input_fn=input, secret_input=getpass):
    manager = runtime.wallet_manager
    command = args.wallet_command

    if command == "list":
        records = manager.list_wallets()
        renderer.render_wallets(records, manager.storage.get_default())
        return records

    if command == "use":
        manager.set_default(args.name)
        renderer.success(f'Default wallet is now "{args.name}"')
        return manager.get_record(args.name)

    if command in {"import-watch", "watch"}:
        record = manager.add_watch(args.name, args.address, network=runtime.network)
        renderer.success(f'Added watch-only wallet "{record.name}"')
        return record

    if command == "import-secret":
        secret = secret_input("Stellar secret (S...): ")
        password = _new_password(secret_input)
        record = manager.import_secret(
            args.name,
            secret,
            password,
            network=runtime.network,
        )
        renderer.success(f'Imported wallet "{record.name}"')
        return record

    if command == "import-mnemonic":
        mnemonic = secret_input("Mnemonic phrase: ")
        mnemonic_passphrase = secret_input(
            "BIP39 passphrase (optional; leave empty if none): "
        )
        password = _new_password(secret_input)
        record = manager.import_mnemonic(
            args.name,
            mnemonic,
            password,
            mnemonic_passphrase=mnemonic_passphrase,
            index=args.index,
            language=args.language,
            network=runtime.network,
        )
        renderer.success(f'Imported wallet "{record.name}"')
        return record

    if command == "create":
        mnemonic = generate_mnemonic_phrase(
            language=args.language,
            strength=args.strength,
        )
        mnemonic_passphrase = secret_input(
            "BIP39 passphrase (optional; leave empty if none): "
        )
        password = _new_password(secret_input)
        record = manager.import_mnemonic(
            args.name,
            mnemonic,
            password,
            mnemonic_passphrase=mnemonic_passphrase,
            index=args.index,
            language=args.language,
            network=runtime.network,
        )
        renderer.render_created_mnemonic(record, mnemonic)
        return record

    if command in {"testnet-fund", "fund"}:
        from .fund import execute_fund

        return execute_fund(runtime, args, renderer)

    if command == "delete":
        answer = input_fn(
            f'Delete wallet "{args.name}" metadata and encrypted secret? [y/N] '
        )
        if answer.strip().lower() != "y":
            return None
        manager.delete(args.name)
        renderer.success(f'Deleted wallet "{args.name}"')
        return None

    raise ValueError(f"Unknown wallet command: {command}")


def _new_password(secret_input) -> str:
    password = secret_input("New wallet password: ")
    confirmation = secret_input("Confirm wallet password: ")
    if password != confirmation:
        raise ValueError("Wallet passwords do not match")
    if not password:
        raise ValueError("Wallet password cannot be empty")
    return password
