use crate::keyring_client::KeyringClient;
use anyhow::Result;
use blake3;
use iota_stronghold::{ClientError, KeyProvider, SnapshotPath, Stronghold};
use rand::distr::{Alphanumeric, SampleString};
use std::path::PathBuf;

/// All per-user session data stored in the stronghold.
pub struct Session {
    pub user_id: String,
    pub device_id: String,
    pub access_token: String,
    pub refresh_token: Option<String>,
}

pub struct SecretService {
    /// Shared keyring client for fetching/creating the stronghold encryption key.
    keyring: KeyringClient,
    /// Directory where per-user stronghold snapshot files are kept.
    stronghold_path: PathBuf,
}

impl SecretService {
    pub fn new(keyring: KeyringClient, stronghold_path: PathBuf) -> Self {
        SecretService { keyring, stronghold_path }
    }

    /// Generate a random 32-character alphanumeric string.
    fn random_secret(&self) -> String {
        Alphanumeric.sample_string(&mut rand::rng(), 32)
    }

    /// Return the blake3 hex hash of `user_id`, used as both the keyring account
    /// name and the stronghold snapshot filename so each user has isolated secrets.
    pub fn user_id_hash(user_id: &str) -> String {
        blake3::hash(user_id.as_bytes()).to_string()
    }

    /// Return the snapshot path for `user_id` (blake3-hashed to be FS-safe).
    fn snapshot_path(&self, user_id: &str) -> SnapshotPath {
        SnapshotPath::from_path(self.stronghold_path.join(Self::user_id_hash(user_id)))
    }

    /// Fetch (or lazily create) the per-user stronghold encryption key from the OS keyring.
    /// The keyring account name is the blake3 hash of `user_id`, giving each user
    /// their own isolated keyring entry.
    fn key_provider(&self, user_id: &str) -> Result<KeyProvider> {
        self.keyring.key_provider(&Self::user_id_hash(user_id))
    }

    /// Open the stronghold store for `user_id`.
    ///
    /// When `create_if_missing` is `true` the client and snapshot are created on first use;
    /// otherwise `None` is returned if either does not yet exist.
    fn open_store(
        &self,
        user_id: &str,
        create_if_missing: bool,
    ) -> Result<Option<(Stronghold, iota_stronghold::Store, KeyProvider, SnapshotPath)>> {
        let key_provider = self.key_provider(user_id)?;
        let snapshot_path = self.snapshot_path(user_id);
        let stronghold = Stronghold::default();

        match stronghold.load_snapshot(&key_provider, &snapshot_path) {
            Ok(()) => {}
            Err(ClientError::SnapshotFileMissing(_)) if create_if_missing => {
                // Snapshot does not exist yet. we will create it on commit.
            }
            Err(ClientError::SnapshotFileMissing(_)) => return Ok(None),
            Err(e) => return Err(e.into()),
        }

        let client = match stronghold.load_client(user_id) {
            Ok(c) => c,
            Err(ClientError::ClientDataNotPresent) if create_if_missing => {
                stronghold.create_client(user_id)?
            }
            Err(ClientError::ClientDataNotPresent) => return Ok(None),
            Err(e) => return Err(e.into()),
        };

        Ok(Some((stronghold, client.store(), key_provider, snapshot_path)))
    }

    /// Commit changes to disk.
    fn commit(
        &self,
        stronghold: &Stronghold,
        key_provider: &KeyProvider,
        snapshot_path: &SnapshotPath,
    ) -> Result<()> {
        Ok(stronghold.commit_with_keyprovider(snapshot_path, key_provider)?)
    }


    /// Persist a full [`Session`] (and lazily create a sqlite password if none exists yet).
    pub fn set_session(&self, session: &Session) -> Result<()> {
        let (stronghold, store, key_provider, snapshot_path) =
            self.open_store(&session.user_id, true)?.unwrap();

        store.insert(b"user_id".to_vec(), session.user_id.as_bytes().to_vec(), None)?;
        store.insert(b"device_id".to_vec(), session.device_id.as_bytes().to_vec(), None)?;
        store.insert(b"access_token".to_vec(), session.access_token.as_bytes().to_vec(), None)?;

        if let Some(t) = &session.refresh_token {
            store.insert(b"refresh_token".to_vec(), t.as_bytes().to_vec(), None)?;
        } else {
            let _ = store.delete(b"refresh_token");
        }

        // Generate a sqlite password on first login and never overwrite it.
        if store.get(b"sqlite_password")?.is_none() {
            let pwd = self.random_secret();
            store.insert(b"sqlite_password".to_vec(), pwd.into_bytes(), None)?;
        }

        self.commit(&stronghold, &key_provider, &snapshot_path)
    }

    /// Retrieve the stored [`Session`] for `user_id`, or `None` if not found.
    pub fn get_session(&self, user_id: &str) -> Result<Option<Session>> {
        let Some((_, store, _, _)) = self.open_store(user_id, false)? else {
            return Ok(None);
        };

        let Some(access_bytes) = store.get(b"access_token")? else {
            return Ok(None);
        };

        Ok(Some(Session {
            user_id: user_id.to_string(),
            device_id: store
                .get(b"device_id")?
                .map(|b| String::from_utf8(b))
                .transpose()?
                .unwrap_or_default(),
            access_token: String::from_utf8(access_bytes)?,
            refresh_token: store
                .get(b"refresh_token")?
                .map(|b| String::from_utf8(b))
                .transpose()?,
        }))
    }

    /// Return the sqlite password for `user_id`, or `None` if no session exists yet.
    pub fn get_sqlite_pwd(&self, user_id: &str) -> Result<Option<String>> {
        let Some((_, store, _, _)) = self.open_store(user_id, false)? else {
            return Ok(None);
        };
        store
            .get(b"sqlite_password")?
            .map(|b| String::from_utf8(b).map_err(Into::into))
            .transpose()
    }

    /// Return the sqlite password for `user_id`, generating and persisting one if it doesn't
    /// exist yet. Always returns a password (creating the stronghold snapshot if needed).
    pub fn get_or_create_sqlite_pwd(&self, user_id: &str) -> Result<String> {
        let (stronghold, store, key_provider, snapshot_path) =
            self.open_store(user_id, true)?.unwrap();

        if let Some(bytes) = store.get(b"sqlite_password")? {
            return Ok(String::from_utf8(bytes)?);
        }

        let pwd = self.random_secret();
        store.insert(b"sqlite_password".to_vec(), pwd.clone().into_bytes(), None)?;
        self.commit(&stronghold, &key_provider, &snapshot_path)?;
        Ok(pwd)
    }
}