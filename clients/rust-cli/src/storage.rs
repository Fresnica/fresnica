use std::fs;
use std::path::{Path, PathBuf};

use fresnica_core::AccountIdentity;
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};
use sha2::{Digest, Sha256};

pub const BACKUP_FORMAT: &str = "fresnica-wallet-backup";
pub const BACKUP_VERSION: u8 = 1;

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct WalletRecord {
    pub name: String,
    pub address: String,
    pub wallet_type: String,
    #[serde(default = "default_network")]
    pub network: String,
    #[serde(default)]
    pub secret: Option<Value>,
    #[serde(default)]
    pub metadata: Map<String, Value>,
}

impl WalletRecord {
    pub fn watch_only(&self) -> bool {
        self.wallet_type == "watch-only"
    }
}

#[derive(Serialize, Deserialize)]
struct WalletBackup {
    format: String,
    version: u8,
    wallet: WalletRecord,
}

pub struct WalletStorage {
    directory: PathBuf,
    default_path: PathBuf,
}

impl WalletStorage {
    pub fn new(home: &Path) -> Result<Self, String> {
        let directory = home.join("wallets");
        fs::create_dir_all(&directory)
            .map_err(|error| format!("unable to create wallet directory: {error}"))?;
        restrict_directory(&directory)?;
        let default_path = directory.join(".default");
        Ok(Self {
            directory,
            default_path,
        })
    }

    pub fn wallet_path(&self, name: &str) -> PathBuf {
        let digest = Sha256::digest(name.as_bytes());
        let mut hex = String::with_capacity(64);
        for byte in digest {
            use std::fmt::Write as _;
            write!(&mut hex, "{byte:02x}").expect("writing to String cannot fail");
        }
        self.directory.join(format!("{hex}.wallet.json"))
    }

    pub fn save(&self, record: &WalletRecord, overwrite: bool) -> Result<(), String> {
        validate_record(record)?;
        let path = self.wallet_path(&record.name);
        if path.exists() && !overwrite {
            return Err(format!("wallet already exists: {}", record.name));
        }
        let text = serde_json::to_string_pretty(record)
            .map_err(|error| format!("unable to encode wallet record: {error}"))?;
        atomic_write(&path, &(text + "\n"))
    }

    pub fn load(&self, name: &str) -> Result<WalletRecord, String> {
        let path = self.wallet_path(name);
        let text = fs::read_to_string(&path)
            .map_err(|_| format!("wallet not found: {name}"))?;
        let record: WalletRecord = serde_json::from_str(&text)
            .map_err(|error| format!("invalid wallet record {name}: {error}"))?;
        if record.name != name {
            return Err(format!("wallet not found: {name}"));
        }
        validate_record(&record)?;
        Ok(record)
    }

    pub fn list(&self) -> Result<Vec<WalletRecord>, String> {
        let mut records = Vec::new();
        let entries = fs::read_dir(&self.directory)
            .map_err(|error| format!("unable to list wallets: {error}"))?;
        for entry in entries {
            let entry = entry.map_err(|error| format!("unable to list wallets: {error}"))?;
            let path = entry.path();
            let Some(name) = path.file_name().and_then(|value| value.to_str()) else {
                continue;
            };
            if !name.ends_with(".wallet.json") {
                continue;
            }
            let text = fs::read_to_string(&path)
                .map_err(|error| format!("unable to read wallet record: {error}"))?;
            let record: WalletRecord = serde_json::from_str(&text)
                .map_err(|error| format!("invalid wallet record {}: {error}", path.display()))?;
            validate_record(&record)?;
            records.push(record);
        }
        records.sort_by_key(|record| record.name.to_lowercase());
        Ok(records)
    }

    pub fn default_name(&self) -> Result<Option<String>, String> {
        if !self.default_path.exists() {
            return Ok(None);
        }
        let value = fs::read_to_string(&self.default_path)
            .map_err(|error| format!("unable to read default wallet: {error}"))?;
        let value = value.trim();
        if value.is_empty() {
            Ok(None)
        } else {
            Ok(Some(value.to_owned()))
        }
    }

    pub fn set_default(&self, name: &str) -> Result<(), String> {
        self.load(name)?;
        atomic_write(&self.default_path, &(name.to_owned() + "\n"))
    }

    pub fn resolve(&self, name: Option<&str>) -> Result<WalletRecord, String> {
        if let Some(name) = name {
            return self.load(name);
        }
        if let Some(name) = self.default_name()? {
            return self.load(&name);
        }
        let records = self.list()?;
        if records.len() == 1 {
            return Ok(records.into_iter().next().expect("one record exists"));
        }
        Err("no default wallet selected".to_owned())
    }

    pub fn delete(&self, name: &str) -> Result<(), String> {
        let path = self.wallet_path(name);
        if !path.exists() {
            return Err(format!("wallet not found: {name}"));
        }
        fs::remove_file(&path).map_err(|error| format!("unable to delete wallet: {error}"))?;
        if self.default_name()?.as_deref() == Some(name) {
            let _ = fs::remove_file(&self.default_path);
        }
        Ok(())
    }

    pub fn write_backup(
        &self,
        record: &WalletRecord,
        destination: &Path,
        overwrite: bool,
    ) -> Result<(), String> {
        validate_record(record)?;
        if destination.exists() && !overwrite {
            return Err(format!("backup file already exists: {}", destination.display()));
        }
        if let Some(parent) = destination.parent() {
            fs::create_dir_all(parent)
                .map_err(|error| format!("unable to create backup directory: {error}"))?;
        }
        let backup = WalletBackup {
            format: BACKUP_FORMAT.to_owned(),
            version: BACKUP_VERSION,
            wallet: record.clone(),
        };
        let text = serde_json::to_string_pretty(&backup)
            .map_err(|error| format!("unable to encode wallet backup: {error}"))?;
        atomic_write(destination, &(text + "\n"))
    }

    pub fn read_backup(path: &Path) -> Result<WalletRecord, String> {
        let text = fs::read_to_string(path)
            .map_err(|error| format!("unable to read wallet backup {}: {error}", path.display()))?;
        let backup: WalletBackup = serde_json::from_str(&text)
            .map_err(|error| format!("invalid wallet backup: {error}"))?;
        if backup.format != BACKUP_FORMAT || backup.version != BACKUP_VERSION {
            return Err("unsupported wallet backup format".to_owned());
        }
        validate_record(&backup.wallet)?;
        Ok(backup.wallet)
    }
}

pub fn validate_record(record: &WalletRecord) -> Result<(), String> {
    if record.name.trim().is_empty() {
        return Err("wallet name cannot be empty".to_owned());
    }
    if !matches!(record.wallet_type.as_str(), "watch-only" | "secret" | "mnemonic") {
        return Err(format!("unsupported wallet type: {}", record.wallet_type));
    }
    if !matches!(record.network.as_str(), "mainnet" | "testnet") {
        return Err(format!("unknown network: {}", record.network));
    }
    let identity = AccountIdentity::parse(&record.address)
        .map_err(|_| "invalid Stellar wallet address".to_owned())?;
    if !identity.is_classic() || identity.address() != record.address {
        return Err("wallet address must be a canonical Classic G address".to_owned());
    }
    if record.watch_only() {
        if record.secret.is_some() {
            return Err("watch-only wallet contains signing material".to_owned());
        }
    } else if !record.secret.as_ref().is_some_and(Value::is_object) {
        return Err("encrypted signing material is missing".to_owned());
    }
    Ok(())
}

fn default_network() -> String {
    "mainnet".to_owned()
}

fn atomic_write(path: &Path, text: &str) -> Result<(), String> {
    let mut temporary_name = path.as_os_str().to_owned();
    temporary_name.push(".tmp");
    let temporary = PathBuf::from(temporary_name);
    let result = (|| {
        fs::write(&temporary, text)
            .map_err(|error| format!("unable to write {}: {error}", path.display()))?;
        restrict_file(&temporary)?;
        #[cfg(windows)]
        if path.exists() {
            fs::remove_file(path)
                .map_err(|error| format!("unable to replace {}: {error}", path.display()))?;
        }
        fs::rename(&temporary, path)
            .map_err(|error| format!("unable to replace {}: {error}", path.display()))?;
        Ok(())
    })();
    if result.is_err() {
        let _ = fs::remove_file(&temporary);
    }
    result
}

#[cfg(unix)]
fn restrict_directory(path: &Path) -> Result<(), String> {
    use std::os::unix::fs::PermissionsExt;
    fs::set_permissions(path, fs::Permissions::from_mode(0o700))
        .map_err(|error| format!("unable to protect wallet directory: {error}"))
}

#[cfg(not(unix))]
fn restrict_directory(_path: &Path) -> Result<(), String> {
    Ok(())
}

#[cfg(unix)]
fn restrict_file(path: &Path) -> Result<(), String> {
    use std::os::unix::fs::PermissionsExt;
    fs::set_permissions(path, fs::Permissions::from_mode(0o600))
        .map_err(|error| format!("unable to protect wallet file: {error}"))
}

#[cfg(not(unix))]
fn restrict_file(_path: &Path) -> Result<(), String> {
    Ok(())
}

#[cfg(test)]
mod tests {
    use std::time::{SystemTime, UNIX_EPOCH};

    use super::*;

    const PUBLIC: &str = "GDLVVGABQKYQVN6VJP7NHSLEA45A5YLS6PNKMIZFV4BBU2HXA5IRVHUR";

    fn temp_home(label: &str) -> PathBuf {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let path = std::env::temp_dir().join(format!(
            "fresnica-rust-cli-{label}-{}-{nonce}",
            std::process::id()
        ));
        fs::create_dir_all(&path).unwrap();
        path
    }

    fn watch_record(name: &str) -> WalletRecord {
        WalletRecord {
            name: name.to_owned(),
            address: PUBLIC.to_owned(),
            wallet_type: "watch-only".to_owned(),
            network: "testnet".to_owned(),
            secret: None,
            metadata: Map::new(),
        }
    }

    #[test]
    fn wallet_filename_matches_python_sha256_rule() {
        let home = temp_home("filename");
        let storage = WalletStorage::new(&home).unwrap();
        let expected = "8ed3f6ad685b959ead7022518e1af76cd816f8e8ec7ccdda1ed4018e8f2223f8.wallet.json";
        assert_eq!(storage.wallet_path("alpha").file_name().unwrap(), expected);
        let _ = fs::remove_dir_all(home);
    }

    #[test]
    fn saves_lists_and_selects_python_compatible_record() {
        let home = temp_home("storage");
        let storage = WalletStorage::new(&home).unwrap();
        let record = watch_record("alpha");
        storage.save(&record, false).unwrap();
        storage.set_default("alpha").unwrap();

        assert_eq!(storage.load("alpha").unwrap(), record);
        assert_eq!(storage.list().unwrap(), vec![record.clone()]);
        assert_eq!(storage.resolve(None).unwrap(), record);
        let _ = fs::remove_dir_all(home);
    }

    #[test]
    fn backup_roundtrips_version_one_record() {
        let home = temp_home("backup");
        let storage = WalletStorage::new(&home).unwrap();
        let record = watch_record("alpha");
        let backup = home.join("alpha.backup.json");

        storage.write_backup(&record, &backup, false).unwrap();
        assert_eq!(WalletStorage::read_backup(&backup).unwrap(), record);
        let _ = fs::remove_dir_all(home);
    }
}
