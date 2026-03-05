use anyhow::Result;
use blake3;
use iota_stronghold::{ClientError, KeyProvider, SnapshotPath, Store, Stronghold};
use keyring::{Entry, Error::NoEntry};
use std::path::PathBuf;

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
        SecretService {
            keyring_service,
            keyring_user,
            stronghold_path,
        }
    }

    fn get_snapshot_path(&self, username: &String) -> SnapshotPath {
        // hash the username so the resulting characters are guaranteed to be an accepted path
        let file_name = blake3::hash(username.as_bytes()).to_string();
        let mut path = self.stronghold_path.clone();
        path.push(file_name);
        SnapshotPath::from_path(path)
    }

    fn generate_password(&self) -> Result<String> {
        let mut buf = [0u8; 32];
        getrandom::fill(&mut buf)?;
        Ok(hex::encode(buf))
    }

    fn get_key_provider(&self) -> Result<KeyProvider> {
        let entry = Entry::new(&self.keyring_service, &self.keyring_user)?;

        let keyring_password = match entry.get_password() {
            Err(NoEntry) => {
                println!("Creating new keyring password");
                let password = self.generate_password()?;
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

    fn get_client_store(&self, username: &String, stronghold: &Stronghold) -> Result<ClientStore> {
        let key_provider = self.get_key_provider()?;
        let snapshot_path = self.get_snapshot_path(&username);
        let snapshot = stronghold.load_snapshot(&key_provider, &snapshot_path);

        match snapshot {
            Ok(()) => {
                let client = match stronghold.load_client(&username) {
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
                    store: Option::from(store),
                })
            }
            Err(ClientError::SnapshotFileMissing(_path)) => {
                Ok(ClientStore {
                    key_provider,
                    snapshot_path,
                    store: None,
                })
            }
            Err(e) => return Err(e.into()),
        }
    }

    fn set_client_store(&self, username: &String, stronghold: &Stronghold) -> Result<ClientStore> {
        match self.get_client_store(&username, stronghold)? {
            client_store @ ClientStore { store: Some(_), .. } => Ok(client_store),
            ClientStore {
                key_provider,
                snapshot_path,
                store: _,
            } => {
                let client = stronghold.create_client(&username)?;
                Ok(ClientStore {
                    key_provider,
                    snapshot_path,
                    store: Some(client.store()),
                })
            }
        }
    }

    pub fn set_login_tokens(&self, username: String, access_token: String, refresh_token: Option<String>) -> Result<()> {
        let stronghold = Stronghold::default();
        let client_store = self.set_client_store(&username, &stronghold)?;
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

        if self.get_sqlite_pwd(username.clone())?.is_none() {
            let password = self.generate_password()?;
            store.insert(b"sqlite_password".to_vec(), Vec::from(password), None)?;
        }

        stronghold.commit_with_keyprovider(&client_store.snapshot_path, &client_store.key_provider)?;

        Ok(())
    }

    pub fn get_login_tokens(&self, username: String) -> Result<Option<(String, Option<String>)>> {
        let stronghold = Stronghold::default();
        match self.get_client_store(&username, &stronghold)? {
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

    pub fn get_sqlite_pwd(&self, username: String) -> Result<Option<String>>{
        let stronghold = Stronghold::default();

        match self.get_client_store(&username, &stronghold)? {
            ClientStore { store : Some(store), .. } => {
                let Some(password) = store.get(b"sqlite_password")? else {
                    return Ok(None);
                };
                Ok(Some(String::from_utf8(password)?))
            },
            ClientStore { store: _, ..} => Ok(None),
        }
    }

    fn set_sqlite_pwd(&self, username: String) -> Result<()> {
        let stronghold = Stronghold::default();
        let password = self.generate_password()?;
        let client_store = self.get_client_store(&username, &stronghold)?;
        let store = client_store.store.unwrap();

        store.insert(b"sqlite_password".to_vec(), Vec::from(password), None)?;
        stronghold.commit_with_keyprovider(&client_store.snapshot_path, &client_store.key_provider)?;
        Ok(())

    }
}