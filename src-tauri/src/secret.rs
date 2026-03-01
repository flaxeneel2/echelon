use iota_stronghold::{KeyProvider, Stronghold, SnapshotPath, ClientError, Store};
use keyring::{Entry, Error::NoEntry};
use std::path::{PathBuf};
use blake3;

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

pub enum SecretError {
    Keyring(keyring::Error),
    Random(getrandom::Error),
    Stronghold(ClientError),
    Utf8(std::string::FromUtf8Error),
}

impl From<keyring::Error> for SecretError {
    fn from(e: keyring::Error) -> Self {
        SecretError::Keyring(e)
    }
}

impl From<getrandom::Error> for SecretError {
    fn from(e: getrandom::Error) -> Self {
        SecretError::Random(e)
    }
}

impl From<ClientError> for SecretError {
    fn from(e: ClientError) -> Self {
        SecretError::Stronghold(e)
    }
}

impl From<std::string::FromUtf8Error> for SecretError {
    fn from(e: std::string::FromUtf8Error) -> Self {
        SecretError::Utf8(e)
    }
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

        println!("Username:{}\nSnapshot Path:{:?}", username, path);
        SnapshotPath::from_path(path)
    }

    fn get_key_provider(&self) -> Result<KeyProvider, SecretError> {
        let entry = Entry::new(&self.keyring_service, &self.keyring_user)?;

        match entry.get_password() {
            Err(NoEntry) => {
                println!("Creating new keyring password");
                let mut buf = [0u8; 32];
                getrandom::fill(&mut buf)?;
                let _ = entry.set_password(&hex::encode(buf));
            },
            Ok(_) => {
                println!("Keyring password set")
            }
            Err(e) => return Err(e.into()),
        }

        // uncomment to delete the keyring password (for testing)
        //println!("{:?}", entry.delete_credential().unwrap());
        let keyring_password = entry.get_password()?;
        Ok(KeyProvider::with_passphrase_hashed_blake2b(keyring_password)?)
    }

    fn get_client_store(&self, username: &String, stronghold: &Stronghold) -> Result<ClientStore, SecretError> {
        let key_provider = self.get_key_provider()?;
        let snapshot_path = self.get_snapshot_path(&username);
        let snapshot = stronghold.load_snapshot(&key_provider, &snapshot_path);

        match snapshot {
            Ok(()) => {
                println!("Successfully loaded snapshot");
            },
            Err(ClientError::SnapshotFileMissing(_path)) => {
                return Ok(ClientStore { key_provider, snapshot_path, store: None });
            },
            Err(e) => return Err(e.into()),
        }


        let client = match stronghold.load_client(&username) {
            Ok(client) => {
                println!("Loaded client successfully");
                client
            }
            Err(ClientError::ClientDataNotPresent) => {
                return Ok(ClientStore { key_provider, snapshot_path, store: None });
            },
            Err(e) => return Err(e.into()),
        };

        let store = client.store();

        Ok(ClientStore { key_provider, snapshot_path, store: Option::from(store) })
    }

    fn set_client_store(&self, username: &String, stronghold: &Stronghold) -> Result<ClientStore, SecretError> {
        match self.get_client_store(&username, stronghold)? {
            client_store @ ClientStore { store: Some(_), .. } => {
                Ok(client_store)
            },
            ClientStore { key_provider, snapshot_path, store: _ } => {
                println!("Creating new snapshot and client store");
                let client = stronghold.create_client(&username)?;
                Ok(ClientStore { key_provider, snapshot_path, store: Some(client.store()) })
            },
        }
    }

    pub fn set_login_tokens(&self, username: String, access_token: String, refresh_token: String) -> Result<(), SecretError> {
        let stronghold = Stronghold::default();
        let client_store = self.set_client_store(&username, &stronghold)?;
        let store = client_store.store.unwrap();
        let key_provider = client_store.key_provider;
        let snapshot_path = client_store.snapshot_path;

        store.insert(b"access_token".to_vec(), Vec::from(access_token), None)?;
        store.insert(b"refresh_token".to_vec(), Vec::from(refresh_token), None)?;

        stronghold.commit_with_keyprovider(&snapshot_path, &key_provider)?;
        Ok(())
    }

    pub fn get_login_tokens(&self, username: String) -> Result<Option<(String, String)>, SecretError> {
        let stronghold = Stronghold::default();
        match self.get_client_store(&username, &stronghold)? {
            client_store @ ClientStore { store: Some(_), .. } => {
                let store = match client_store.store {
                    Some(store) => store,
                    None => return Ok(None),
                };

                let access_token = String::from_utf8(store.get(b"access_token")?.unwrap());
                let refresh_token = String::from_utf8(store.get(b"refresh_token")?.unwrap());

                Ok(Some((access_token?, refresh_token?)))
            },
            ClientStore { store: _, .. } => {
                Ok(None)
            }
        }
    }
}