"""Local contact/address book commands."""


def execute_contact(runtime, args, renderer):
    store = runtime.contact_store
    command = args.contact_command

    if command == "list":
        contacts = store.list()
        renderer.render_contacts(contacts)
        return contacts

    if command == "add":
        contact = store.add(args.name, args.address, memo=args.memo)
        renderer.success(f'Added contact "{contact.name}"')
        return contact

    if command == "remove":
        contact = store.remove(args.name)
        renderer.success(f'Removed contact "{contact.name}"')
        return contact

    raise ValueError(f"Unknown contact command: {command}")
