"""Wallet lifecycle commands."""

from getpass import getpass


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
        password = _app_passcode(manager, secret_input)
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
        password = _app_passcode(manager, secret_input)
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
        mnemonic_passphrase = secret_input(
            "BIP39 passphrase (optional; leave empty if none): "
        )
        password = _app_passcode(manager, secret_input)
        record, mnemonic = manager.create_mnemonic(
            args.name,
            password,
            mnemonic_passphrase=mnemonic_passphrase,
            index=args.index,
            language=args.language,
            strength=args.strength,
            network=runtime.network,
        )
        renderer.render_created_mnemonic(record, mnemonic)
        return record

    if command == "backup":
        path = manager.backup(args.name, args.path, overwrite=args.force)
        renderer.success(
            f'Encrypted backup for "{args.name}" written to {path}; Fresnica passcode is unchanged'
        )
        return path

    if command == "restore":
        password = None
        if manager.has_app_passcode():
            password = _existing_app_passcode(secret_input)
        record = manager.restore_backup(
            args.path,
            name=args.name,
            wallet_password=password,
        )
        renderer.success(
            f'Restored wallet "{record.name}" [{record.network}]; unlock with the Fresnica passcode'
        )
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


def _app_passcode(manager, secret_input) -> str:
    if manager.has_app_passcode():
        return _existing_app_passcode(secret_input)
    return _new_app_passcode(secret_input)


def _existing_app_passcode(secret_input) -> str:
    password = secret_input("Fresnica passcode: ")
    if not password:
        raise ValueError("Fresnica passcode cannot be empty")
    return password


def _new_app_passcode(secret_input) -> str:
    password = secret_input("Create Fresnica passcode: ")
    confirmation = secret_input("Confirm Fresnica passcode: ")
    if password != confirmation:
        raise ValueError("Fresnica passcodes do not match")
    if not password:
        raise ValueError("Fresnica passcode cannot be empty")
    return password
