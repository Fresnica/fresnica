use fresnica_client::{ContactStore, WalletStorage};

pub fn command_contact(storage: &WalletStorage, arguments: &[String]) -> Result<(), String> {
    let store = ContactStore::for_home(storage.home());
    let Some(command) = arguments.first().map(String::as_str) else {
        return Err(usage().to_owned());
    };
    match command {
        "list" if arguments.len() == 1 => {
            let contacts = store.list()?;
            if contacts.is_empty() {
                println!("No local contacts.");
                return Ok(());
            }
            for contact in contacts {
                if let Some(memo) = contact.memo.as_deref() {
                    println!("{:<24} {}  memo={memo}", contact.name, contact.address);
                } else {
                    println!("{:<24} {}", contact.name, contact.address);
                }
            }
            Ok(())
        }
        "add" => {
            if arguments.len() < 3 {
                return Err(usage().to_owned());
            }
            let mut memo = None;
            let mut index = 3;
            while index < arguments.len() {
                if arguments[index] != "--memo" || memo.is_some() {
                    return Err(usage().to_owned());
                }
                index += 1;
                memo = Some(
                    arguments
                        .get(index)
                        .ok_or_else(|| usage().to_owned())?
                        .as_str(),
                );
                index += 1;
            }
            let contact = store.add(&arguments[1], &arguments[2], memo)?;
            println!("Added contact \"{}\"", contact.name);
            println!("Address: {}", contact.address);
            if let Some(memo) = contact.memo {
                println!("Memo:    {memo}");
            }
            Ok(())
        }
        "remove" if arguments.len() == 2 => {
            let contact = store.remove(&arguments[1])?;
            println!("Removed contact \"{}\"", contact.name);
            Ok(())
        }
        _ => Err(usage().to_owned()),
    }
}

fn usage() -> &'static str {
    "usage: fresnica contact list | contact add NAME G... [--memo TEXT] | contact remove NAME"
}
