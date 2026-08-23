from types import SimpleNamespace

from stellar_sdk import Keypair

from fresnica.cli.commands.contact import execute_contact
from fresnica.contacts import ContactStore


class Renderer:
    def __init__(self):
        self.messages = []
        self.contacts = None

    def success(self, message):
        self.messages.append(message)

    def render_contacts(self, contacts):
        self.contacts = contacts


def test_contact_cli_add_list_remove(tmp_path):
    runtime = SimpleNamespace(contact_store=ContactStore(tmp_path / "contacts.json"))
    renderer = Renderer()
    address = Keypair.random().public_key

    added = execute_contact(
        runtime,
        SimpleNamespace(
            contact_command="add",
            name="Alice",
            address=address,
            memo="42",
        ),
        renderer,
    )
    listed = execute_contact(
        runtime,
        SimpleNamespace(contact_command="list"),
        renderer,
    )
    removed = execute_contact(
        runtime,
        SimpleNamespace(contact_command="remove", name="alice"),
        renderer,
    )

    assert added.address == address
    assert listed == [added]
    assert renderer.contacts == [added]
    assert removed == added
    assert runtime.contact_store.list() == []
    assert renderer.messages == [
        'Added contact "Alice"',
        'Removed contact "Alice"',
    ]
