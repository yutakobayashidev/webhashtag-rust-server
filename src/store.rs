use std::fs;
use std::path::Path;
use std::sync::{Arc, Mutex};

use chrono::{SecondsFormat, Utc};
use redb::{Database, ReadableDatabase, ReadableTable, TableDefinition};
use serde::{Deserialize, Serialize};

use crate::atom::build_atom_feed;
use crate::verify::OgpData;

const ENTRIES: TableDefinition<&str, &[u8]> = TableDefinition::new("entries");
const KEY_SEPARATOR: char = '\0';

type StoreResult<T> = Result<T, Box<dyn std::error::Error + Send + Sync>>;

#[derive(Clone)]
pub struct Store {
    server_host: Arc<Mutex<String>>,
    db: Arc<Database>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Entry {
    pub url: String,
    pub registered_at: String,
    pub ogp: OgpData,
}

impl Store {
    pub fn load(root: impl AsRef<Path>) -> StoreResult<Self> {
        let root = root.as_ref().to_path_buf();
        fs::create_dir_all(root.join("data"))?;
        let db = Database::create(root.join("data/webhashtag.redb"))?;
        let write_txn = db.begin_write()?;
        {
            write_txn.open_table(ENTRIES)?;
        }
        write_txn.commit()?;

        Ok(Self {
            server_host: Arc::new(Mutex::new(String::new())),
            db: Arc::new(db),
        })
    }

    pub fn set_server_host(&self, server_host: &str) {
        *self.server_host.lock().expect("server host lock") = server_host.to_string();
    }

    pub fn entries(&self, tag: &str) -> Option<Vec<Entry>> {
        let prefix = entry_key_prefix(tag);
        let read_txn = self.db.begin_read().ok()?;
        let table = read_txn.open_table(ENTRIES).ok()?;
        let mut entries = Vec::new();
        for item in table.iter().ok()? {
            let (key, value) = item.ok()?;
            if key.value().starts_with(&prefix) {
                entries.push(serde_json::from_slice(value.value()).ok()?);
            }
        }

        if entries.is_empty() {
            None
        } else {
            Some(entries)
        }
    }

    pub fn atom(&self, tag: &str) -> Option<String> {
        let entries = self.entries(tag)?;
        Some(build_atom_feed(tag, &self.server_host(), &entries))
    }

    pub fn has_entry(&self, tag: &str, url: &str) -> bool {
        let Ok(read_txn) = self.db.begin_read() else {
            return false;
        };
        let Ok(table) = read_txn.open_table(ENTRIES) else {
            return false;
        };
        table
            .get(entry_key(tag, url).as_str())
            .is_ok_and(|entry| entry.is_some())
    }

    pub fn add_entry(&self, tag: &str, url: &str, ogp: OgpData) -> StoreResult<()> {
        let entry = Entry {
            url: url.to_string(),
            registered_at: Utc::now().to_rfc3339_opts(SecondsFormat::Secs, true),
            ogp,
        };
        let encoded = serde_json::to_vec(&entry)?;
        let write_txn = self.db.begin_write()?;
        {
            let mut table = write_txn.open_table(ENTRIES)?;
            table.insert(entry_key(tag, url).as_str(), encoded.as_slice())?;
        }
        write_txn.commit()?;
        Ok(())
    }

    pub fn remove_entry(&self, tag: &str, url: &str) -> StoreResult<()> {
        let write_txn = self.db.begin_write()?;
        {
            let mut table = write_txn.open_table(ENTRIES)?;
            table.remove(entry_key(tag, url).as_str())?;
        }
        write_txn.commit()?;
        Ok(())
    }

    pub fn all_tag_entries(&self) -> Vec<(String, String)> {
        let Ok(read_txn) = self.db.begin_read() else {
            return Vec::new();
        };
        let Ok(table) = read_txn.open_table(ENTRIES) else {
            return Vec::new();
        };

        table
            .iter()
            .ok()
            .into_iter()
            .flat_map(|items| items.filter_map(Result::ok))
            .filter_map(|(key, _)| split_entry_key(key.value()))
            .collect()
    }

    fn server_host(&self) -> String {
        self.server_host.lock().expect("server host lock").clone()
    }
}

fn entry_key(tag: &str, url: &str) -> String {
    format!("{tag}{KEY_SEPARATOR}{url}")
}

fn entry_key_prefix(tag: &str) -> String {
    format!("{tag}{KEY_SEPARATOR}")
}

fn split_entry_key(key: &str) -> Option<(String, String)> {
    let (tag, url) = key.split_once(KEY_SEPARATOR)?;
    Some((tag.to_string(), url.to_string()))
}
