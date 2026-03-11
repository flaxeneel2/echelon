use anyhow::Result;
use blake3;
use iota_stronghold::{ClientError, KeyProvider, SnapshotPath, Stronghold};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

use crate::keyring_client::KeyringClient;

/// The list of known accounts persisted on-device.
#[derive(Serialize, Deserialize, Default)]
pub struct Accounts {
    pub(crate) last: Option<String>,
    pub(crate) accounts: Vec<String>,
}

/// App-level persistent store backed by a Stronghold snapshot.
///
/// The encryption key for the snapshot is retrieved from (or lazily created
/// in) the OS keyring via [KeyringClient].
pub struct EchelonStore {
    keyring: KeyringClient,
    /// Keyring account name under which the stronghold key lives.
    keyring_account: String,
    /// Path to the stronghold snapshot file used for app-level data.
    snapshot_path: SnapshotPath,
}

/// Fixed client name inside the stronghold snapshot for app-level data.
const APP_CLIENT: &str = "echelon-app";

impl EchelonStore {
    pub fn new(keyring: KeyringClient, keyring_account: String, store_dir: PathBuf) -> Self {
        // Hash the keyring_account string to get a stable, FS-safe filename.
        let name = blake3::hash(keyring_account.as_bytes()).to_string();
        let snapshot_path = SnapshotPath::from_path(store_dir.join(format!("{name}.stronghold")));
        EchelonStore { keyring, keyring_account, snapshot_path }
    }

    fn key_provider(&self) -> Result<KeyProvider> {
        self.keyring.key_provider(&self.keyring_account)
    }

    /// Open (or lazily create) the stronghold and return its Store.
    fn open(&self) -> Result<(Stronghold, iota_stronghold::Store, KeyProvider)> {
        let key_provider = self.key_provider()?;
        let stronghold = Stronghold::default();

        match stronghold.load_snapshot(&key_provider, &self.snapshot_path) {
            Ok(()) => {}
            Err(ClientError::SnapshotFileMissing(_)) => {
                // First run – snapshot will be created on commit.
            }
            Err(e) => return Err(e.into()),
        }

        let client = match stronghold.load_client(APP_CLIENT) {
            Ok(c) => c,
            Err(ClientError::ClientDataNotPresent) => stronghold.create_client(APP_CLIENT)?,
            Err(e) => return Err(e.into()),
        };

        Ok((stronghold, client.store(), key_provider))
    }

    fn commit(&self, stronghold: &Stronghold, key_provider: &KeyProvider) -> Result<()> {
        Ok(stronghold.commit_with_keyprovider(&self.snapshot_path, key_provider)?)
    }

    fn read_accounts(&self, store: &iota_stronghold::Store) -> Result<Accounts> {
        match store.get(b"accounts")? {
            Some(bytes) => Ok(serde_json::from_slice(&bytes)?),
            None => Ok(Accounts::default()),
        }
    }

    fn write_accounts(
        &self,
        store: &iota_stronghold::Store,
        accounts: &Accounts,
    ) -> Result<()> {
        let bytes = serde_json::to_vec(accounts)?;
        store.insert(b"accounts".to_vec(), bytes, None)?;
        Ok(())
    }

    /// Return a snapshot of all persisted accounts.
    pub fn get_accounts(&self) -> Result<Accounts> {
        let (_, store, _) = self.open()?;
        self.read_accounts(&store)
    }

    /// Add `user_id` to the persisted account list (or move it to front) and
    /// mark it as the most-recently-used account.
    pub fn add_account(&self, user_id: &str) -> Result<()> {
        let (stronghold, store, key_provider) = self.open()?;
        let mut accounts = self.read_accounts(&store)?;

        accounts.accounts.retain(|x| x != user_id);
        accounts.accounts.insert(0, user_id.to_string());
        accounts.last = Some(user_id.to_string());

        self.write_accounts(&store, &accounts)?;
        self.commit(&stronghold, &key_provider)
    }

    /// Remove `user_id` from the persisted account list.
    ///
    /// If it was also the last-used account, promotes the next account in the
    /// list (or sets `last` to `None` if the list is now empty).
    pub fn remove_account(&self, user_id: &str) -> Result<()> {
        let (stronghold, store, key_provider) = self.open()?;
        let mut accounts = self.read_accounts(&store)?;

        let before = accounts.accounts.len();
        accounts.accounts.retain(|x| x != user_id);

        if accounts.accounts.len() == before {
            // Nothing changed – user_id was not in the list.
            return Ok(());
        }

        if accounts.last.as_deref() == Some(user_id) {
            accounts.last = accounts.accounts.first().cloned();
        }

        self.write_accounts(&store, &accounts)?;
        self.commit(&stronghold, &key_provider)
    }

    /// Return the most-recently-used account, if any.
    pub fn get_last(&self) -> Result<Option<String>> {
        Ok(self.get_accounts()?.last)
    }
}