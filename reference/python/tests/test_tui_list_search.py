import asyncio

from stellar_sdk import Keypair
from textual.app import App
from textual.widgets import DataTable

from fresnica.contacts import ContactStore
from fresnica.tui.contact_book import ContactBookScreen
from fresnica.tui.list_search import ListSearchDialog, matches_query


def test_matches_query_is_case_insensitive_across_fields():
    assert matches_query("xrp", "XRP", "fchain.io")
    assert matches_query("fchain", "XRP", "fchain.io")
    assert not matches_query("aqua", "XRP", "fchain.io")


def test_contact_slash_search_filters_visible_selection_safely(tmp_path):
    async def scenario():
        store = ContactStore(tmp_path / "contacts.json")
        store.add("Alice", Keypair.random().public_key)
        store.add("Bob", Keypair.random().public_key)
        app = App()
        async with app.run_test(size=(100, 30)) as pilot:
            screen = ContactBookScreen(store)
            app.push_screen(screen)
            await pilot.pause(0.1)
            table = screen.query_one("#contacts-table", DataTable)
            assert table.row_count == 2

            await pilot.press("/")
            await pilot.pause(0.05)
            assert isinstance(app.screen, ListSearchDialog)
            await pilot.press("b", "o", "b")
            await pilot.pause(0.05)
            assert table.row_count == 1
            assert screen._contacts[0].name == "Bob"

            await pilot.press("enter")
            await pilot.pause(0.05)
            assert app.screen is screen
            assert table.row_count == 1

            await pilot.press("/")
            await pilot.pause(0.03)
            await pilot.press("escape")
            await pilot.pause(0.05)
            assert table.row_count == 2
            assert [item.name for item in screen._contacts] == ["Alice", "Bob"]

    asyncio.run(scenario())
