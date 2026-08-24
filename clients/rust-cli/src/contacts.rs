use std::ffi::OsString;
use std::fs;
use std::path::{Path, PathBuf};

use fresnica_core::AccountIdentity;
use serde::{Deserialize, Serialize};

use crate::storage::WalletStorage;

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct Contact {
    pub name: String,
    pub address: String,
    #[serde(default)]
    pub memo: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ResolvedDestination {
    pub address: String,
    pub memo: Option<String>,
    pub contact_name: Option<String>,
}

pub struct ContactStore {
    path: PathBuf,
}

impl ContactStore {
    pub fn for_home(home: &Path) -> Self {
        Self {
            path: home.join("contacts.json"),
        }
    }

    pub fn list(&self) -> Result<Vec<Contact>, String> {
        let mut contacts = self.load()?;
        contacts.sort_by_key(|contact| name_key(&contact.name));
        Ok(contacts)
    }

    pub fn find(&self, name: &str) -> Result<Option<Contact>, String> {
        let key = name_key_checked(name)?;
        Ok(self
            .load()?
            .into_iter()
            .find(|contact| name_key(&contact.name) == key))
    }

    pub fn add(&self, name: &str, address: &str, memo: Option<&str>) -> Result<Contact, String> {
        let contact = normalize_contact(name, address, memo)?;
        let mut contacts = self.load()?;
        let key = name_key(&contact.name);
        if contacts
            .iter()
            .any(|existing| name_key(&existing.name) == key)
        {
            return Err(format!("Contact already exists: {}", contact.name));
        }
        contacts.push(contact.clone());
        self.save(&contacts)?;
        Ok(contact)
    }

    pub fn remove(&self, name: &str) -> Result<Contact, String> {
        let key = name_key_checked(name)?;
        let mut contacts = self.load()?;
        let index = contacts
            .iter()
            .position(|contact| name_key(&contact.name) == key)
            .ok_or_else(|| format!("Contact not found: {name}"))?;
        let removed = contacts.remove(index);
        self.save(&contacts)?;
        Ok(removed)
    }

    fn load(&self) -> Result<Vec<Contact>, String> {
        if !self.path.exists() {
            return Ok(Vec::new());
        }
        let text = fs::read_to_string(&self.path)
            .map_err(|_| format!("Unable to read contacts: {}", self.path.display()))?;
        let raw: Vec<Contact> = serde_json::from_str(&text)
            .map_err(|_| format!("Unable to read contacts: {}", self.path.display()))?;
        let mut contacts = Vec::with_capacity(raw.len());
        let mut seen = Vec::with_capacity(raw.len());
        for value in raw {
            let contact = normalize_contact(&value.name, &value.address, value.memo.as_deref())
                .map_err(|_| "Contact address book is malformed".to_owned())?;
            let key = name_key(&contact.name);
            if seen.iter().any(|item| item == &key) {
                return Err("Contact address book contains duplicate names".to_owned());
            }
            seen.push(key);
            contacts.push(contact);
        }
        Ok(contacts)
    }

    fn save(&self, contacts: &[Contact]) -> Result<(), String> {
        if let Some(parent) = self.path.parent() {
            fs::create_dir_all(parent)
                .map_err(|_| format!("Unable to write contacts: {}", self.path.display()))?;
        }
        let text = serde_json::to_string_pretty(contacts)
            .map_err(|_| format!("Unable to write contacts: {}", self.path.display()))?
            + "\n";
        let mut temporary_name: OsString = self.path.as_os_str().to_owned();
        temporary_name.push(".tmp");
        let temporary = PathBuf::from(temporary_name);
        let result = (|| {
            fs::write(&temporary, text)
                .map_err(|_| format!("Unable to write contacts: {}", self.path.display()))?;
            restrict_file(&temporary)?;
            #[cfg(windows)]
            if self.path.exists() {
                fs::remove_file(&self.path)
                    .map_err(|_| format!("Unable to write contacts: {}", self.path.display()))?;
            }
            fs::rename(&temporary, &self.path)
                .map_err(|_| format!("Unable to write contacts: {}", self.path.display()))?;
            Ok(())
        })();
        if result.is_err() {
            let _ = fs::remove_file(&temporary);
        }
        result
    }
}

pub fn resolve_destination(
    storage: &WalletStorage,
    destination: &str,
    explicit_memo: Option<&str>,
) -> Result<ResolvedDestination, String> {
    let store = ContactStore::for_home(storage.home());
    let Some(contact) = store.find(destination)? else {
        return Ok(ResolvedDestination {
            address: destination.to_owned(),
            memo: explicit_memo.map(str::to_owned),
            contact_name: None,
        });
    };
    Ok(ResolvedDestination {
        address: contact.address,
        memo: explicit_memo
            .map(str::to_owned)
            .or_else(|| contact.memo.clone()),
        contact_name: Some(contact.name),
    })
}

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

fn normalize_contact(name: &str, address: &str, memo: Option<&str>) -> Result<Contact, String> {
    let name = name.trim();
    if name.is_empty() {
        return Err("Contact name cannot be empty".to_owned());
    }
    let address = address.trim();
    let identity = AccountIdentity::parse(address)
        .map_err(|_| "Contact address is not a valid Stellar G... account".to_owned())?;
    if !identity.is_classic() || identity.address() != address {
        return Err("Contact address is not a valid Stellar G... account".to_owned());
    }
    let memo = memo
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_owned);
    Ok(Contact {
        name: name.to_owned(),
        address: address.to_owned(),
        memo,
    })
}

fn name_key_checked(name: &str) -> Result<String, String> {
    if name.trim().is_empty() {
        return Err("Contact name cannot be empty".to_owned());
    }
    Ok(name_key(name))
}

fn name_key(name: &str) -> String {
    name.trim().to_lowercase()
}

fn usage() -> &'static str {
    "usage: fresnica contact list | contact add NAME G... [--memo TEXT] | contact remove NAME"
}

#[cfg(unix)]
fn restrict_file(path: &Path) -> Result<(), String> {
    use std::os::unix::fs::PermissionsExt;
    fs::set_permissions(path, fs::Permissions::from_mode(0o600))
        .map_err(|_| "Unable to protect contacts file".to_owned())
}

#[cfg(not(unix))]
fn restrict_file(_path: &Path) -> Result<(), String> {
    Ok(())
}

#[cfg(test)]
mod tests {
    use std::time::{SystemTime, UNIX_EPOCH};

    use super::*;

    const ALICE: &str = "GAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAWHF";
    const BOB: &str = "GDLVVGABQKYQVN6VJP7NHSLEA45A5YLS6PNKMIZFV4BBU2HXA5IRVHUR";

    fn store(label: &str) -> ContactStore {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        ContactStore::for_home(&std::env::temp_dir().join(format!(
            "fresnica-contacts-{label}-{}-{nonce}",
            std::process::id()
        )))
    }

    #[test]
    fn persists_python_compatible_contacts_and_case_insensitive_names() {
        let store = store("roundtrip");
        let contact = store.add(" Alice ", ALICE, Some(" 12345 ")).unwrap();
        assert_eq!(contact.name, "Alice");
        assert_eq!(contact.memo.as_deref(), Some("12345"));
        assert_eq!(store.find("alice").unwrap(), Some(contact.clone()));
        assert!(store.add("ALICE", BOB, None).is_err());

        let raw: serde_json::Value =
            serde_json::from_str(&fs::read_to_string(&store.path).unwrap()).unwrap();
        assert_eq!(raw[0]["name"], "Alice");
        assert_eq!(raw[0]["address"], ALICE);
        assert_eq!(raw[0]["memo"], "12345");
    }

    #[test]
    fn accepts_python_contact_without_memo_field() {
        let store = store("missing-memo");
        fs::create_dir_all(store.path.parent().unwrap()).unwrap();
        fs::write(
            &store.path,
            format!(r#"[{{"name":"Alice","address":"{ALICE}"}}]"#),
        )
        .unwrap();
        let contact = store.find("alice").unwrap().unwrap();
        assert_eq!(contact.address, ALICE);
        assert_eq!(contact.memo, None);
    }

    #[test]
    fn removes_contacts_and_rejects_invalid_addresses() {
        let store = store("remove");
        store.add("Bob", BOB, None).unwrap();
        assert_eq!(store.remove("bob").unwrap().name, "Bob");
        assert!(store.list().unwrap().is_empty());
        assert!(store.add("bad", "not-an-address", None).is_err());
    }

    #[test]
    fn destination_resolution_prefers_explicit_memo() {
        let store = store("resolve");
        let home = store.path.parent().unwrap().to_path_buf();
        store.add("Alice", ALICE, Some("default-memo")).unwrap();
        let storage = WalletStorage::new(&home).unwrap();

        let resolved = resolve_destination(&storage, "ALICE", None).unwrap();
        assert_eq!(resolved.address, ALICE);
        assert_eq!(resolved.memo.as_deref(), Some("default-memo"));
        assert_eq!(resolved.contact_name.as_deref(), Some("Alice"));

        let explicit = resolve_destination(&storage, "alice", Some("explicit")).unwrap();
        assert_eq!(explicit.memo.as_deref(), Some("explicit"));
    }
}
