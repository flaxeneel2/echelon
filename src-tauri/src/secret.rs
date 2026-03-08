use anyhow::Result;
use blake3;
use iota_stronghold::{ClientError, KeyProvider, SnapshotPath, Store, Stronghold};
use keyring::{Entry, Error::NoEntry};
use std::path::PathBuf;
use rand::distr::{Alphanumeric, SampleString};

pub struct SecretService {
    keyring_service: String,
    keyring_user: String,
    stronghold_path: PathBuf,
}

struct ClientStore {
    key_provider: KeyProvider,
    snapshot_path: SnapshotPath,
    store: Option<Store>,
}

impl SecretService {
    pub fn new(keyring_service: String, keyring_user: String, stronghold_path: PathBuf) -> Self {
        #[cfg(target_os= "android")] {
            Self::init_android_keyring();
        }
        SecretService {
            keyring_service,
            keyring_user,
            stronghold_path,
        }
    }

    #[cfg(target_os = "android")]
    fn init_android_keyring() {
        // This connects the keyring crate to Android's Keystore/SharedPreferences
        keyring_core::set_default_store(
            android_native_keyring_store::AndroidStore::from_ndk_context().unwrap()
        );
    }

    /// Return the path of the snapshot as a [SnapshotPath] object.
    ///
    /// # Arguments
    /// * `user_id`: The Matrix account's userid in the format of `@username:homeserver`.
    ///
    /// Given that a Matrix userid is formatted as '@username:homeserver', it won't be accepted as a path for some operating systems, so instead the path for each user's snapshot is a hash of their userid instead.
    fn get_snapshot_path(&self, user_id: &String) -> SnapshotPath {
        let file_name = blake3::hash(user_id.as_bytes()).to_string();
        let mut path = self.stronghold_path.clone();
        path.push(file_name);
        SnapshotPath::from_path(path)
    }

    /// Generate a random password of 32 alphanumeric characters, this is used for both the keyring password and the sqlite password.
    fn generate_password(&self) -> String {
        Alphanumeric.sample_string(&mut rand::rng(), 32)
    }

    /// Access the password from the operating system's keyring and return it as a [KeyProvider].
    ///
    /// If a keyring entry does not exist, then generate one and set it first.
    fn get_key_provider(&self) -> Result<KeyProvider> {
        let entry = Entry::new(&self.keyring_service, &self.keyring_user)?;

        let keyring_password = match entry.get_password() {
            Err(NoEntry) => {
                println!("Creating new keyring password");
                let password = self.generate_password();
                entry.set_password(&password)?;
                password
            }
            Ok(password) => { password }
            Err(e) => return Err(e.into()),
        };

        Ok(KeyProvider::with_passphrase_hashed_blake2b(
            keyring_password,
        )?)
    }

    /// Return a [ClientStore] object, you need this to set/get/remove items for that specific user's store.
    ///
    /// # Arguments
    /// * * `user_id`: The Matrix account's userid in the format of `@username:homeserver`.
    /// * `stronghold` - The in-memory stronghold object being used.
    ///
    /// If a snapshot doesn't already exist, or if there is no client in the snapshot, return a [ClientStore] with the [ClientStore::store] value as [None].
    fn get_client_store(&self, user_id: &String, stronghold: &Stronghold) -> Result<ClientStore> {
        let key_provider = self.get_key_provider()?;
        let snapshot_path = self.get_snapshot_path(&user_id);
        let snapshot = stronghold.load_snapshot(&key_provider, &snapshot_path);

        match snapshot {
            Ok(()) => {
                let client = match stronghold.load_client(&user_id) {
                    Ok(client) => {
                        client
                    }
                    Err(ClientError::ClientDataNotPresent) => {
                        return Ok(ClientStore {
                            key_provider,
                            snapshot_path,
                            store: None,
                        });
                    }
                    Err(e) => return Err(e.into()),
                };

                let store = client.store();

                Ok(ClientStore {
                    key_provider,
                    snapshot_path,
                    store: Some(store),
                })
            }
            Err(ClientError::SnapshotFileMissing(_path)) => {
                Ok(ClientStore {
                    key_provider,
                    snapshot_path,
                    store: None,
                })
            }
            Err(e) => Err(e.into()),
        }
    }

    /// If a [ClientStore] with a [ClientStore::store] that is not [None] exists for that user, return it, otherwise, make one and then return the updated [ClientStore].
    ///
    /// # Arguments
    /// * * `user_id`: The Matrix account's userid in the format of `@username:homeserver`.
    /// * `stronghold` - The in-memory stronghold object being used.
    fn set_client_store(&self, user_id: &String, stronghold: &Stronghold) -> Result<ClientStore> {
        match self.get_client_store(&user_id, stronghold)? {
            client_store @ ClientStore { store: Some(_), .. } => Ok(client_store),
            ClientStore {
                key_provider,
                snapshot_path,
                store: _,
            } => {
                let client = stronghold.create_client(&user_id)?;
                Ok(ClientStore {
                    key_provider,
                    snapshot_path,
                    store: Some(client.store()),
                })
            }
        }
    }


    /// Set the values of the `access_token` and `refresh_token` of the user and save in the stronghold, also sets a password for that user's sqlite database.
    ///
    /// # Arguments
    /// * `user_id`: The Matrix account's userid in the format of `@username:homeserver`.
    ///
    /// Given that a refresh token is optional, if one is not provided, then the value [None] will be stored instead.
    pub fn set_login_tokens(&self, user_id: String, access_token: String, refresh_token: Option<String>) -> Result<()> {
        let stronghold = Stronghold::default();
        let client_store = self.set_client_store(&user_id, &stronghold)?;
        let store = client_store.store.unwrap();

        store.insert(b"access_token".to_vec(), Vec::from(access_token), None)?;

        match refresh_token {
            Some(token) => {
                store.insert(b"refresh_token".to_vec(), Vec::from(token), None)?;

            },
            None => {
                store.delete(b"refresh_token")?;
            }
        }

        if self.get_sqlite_pwd(user_id.clone())?.is_none() {
            let password = self.generate_password();
            store.insert(b"sqlite_password".to_vec(), Vec::from(password), None)?;
        }

        stronghold.commit_with_keyprovider(&client_store.snapshot_path, &client_store.key_provider)?;

        Ok(())
    }

    /// Returns the `access_token` and `refresh_token` stored in the stronghold for a user.
    ///
    /// # Arguments
    /// * `user_id`: The Matrix account's userid in the format of `@username:homeserver`.
    ///
    /// If the given user is not in the store, return [None].
    pub fn get_login_tokens(&self, user_id: String) -> Result<Option<(String, Option<String>)>> {
        let stronghold = Stronghold::default();
        match self.get_client_store(&user_id, &stronghold)? {
            ClientStore { store: Some(store), .. } => {
                let Some(access) = store.get(b"access_token")? else {
                    return Ok(None);
                };

                let access_token = String::from_utf8(access)?;
                let refresh_token = match store.get(b"refresh_token")? {
                    Some(refresh) => Some(String::from_utf8(refresh)?),
                    None => None,
                };

                Ok(Some((access_token, refresh_token)))
            }
            ClientStore { store: _, .. } => Ok(None),
        }
    }

    /// Returns the `sqlite_password` stored in the stronghold for a user.
    ///
    /// # Arguments
    /// * `user_id`: The Matrix account's userid in the format of `@username:homeserver`.
    ///
    /// If the given user is not in the store, return [None].
    pub fn get_sqlite_pwd(&self, user_id: String) -> Result<Option<String>>{
        let stronghold = Stronghold::default();

        match self.get_client_store(&user_id, &stronghold)? {
            ClientStore { store : Some(store), .. } => {
                let Some(password) = store.get(b"sqlite_password")? else {
                    return Ok(None);
                };
                Ok(Some(String::from_utf8(password)?))
            },
            ClientStore { store: _, ..} => Ok(None),
        }
    }

    /// Sets the `sqlite_password` stored in the stronghold for a user.
    ///
    /// # Argument
    /// * `user_id`: The Matrix account's userid in the format of `@username:homeserver`.
    fn set_sqlite_pwd(&self, user_id: String) -> Result<()> {
        let stronghold = Stronghold::default();
        let password = self.generate_password();
        let client_store = self.get_client_store(&user_id, &stronghold)?;
        let store = client_store.store.unwrap();

        store.insert(b"sqlite_password".to_vec(), Vec::from(password), None)?;
        stronghold.commit_with_keyprovider(&client_store.snapshot_path, &client_store.key_provider)?;
        Ok(())

    }
}