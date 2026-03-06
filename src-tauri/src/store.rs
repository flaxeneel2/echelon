use anyhow::Result;
use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Manager, Wry};
use tauri_plugin_store::{Store, StoreExt};
use std::sync::Arc;

#[derive(Serialize, Deserialize)]
pub struct Accounts {
    pub(crate) last : Option<String>,
    pub(crate) accounts : Vec<String>,
}

pub struct EchelonStore {
    store: Arc<Store<Wry>>,
}

impl EchelonStore {
    pub fn new (app_handle: &AppHandle) -> Result<Self> {
        let app_data_dir = app_handle.path().app_data_dir()?;
        let mut path = app_data_dir.clone();
        path.push("store.json");

        let store = app_handle.store(path)?;
        Ok(EchelonStore { store, })
    }

    /// Create an [Accounts] object that gets the values from the store.
    ///
    /// If the store is empty, then return placeholder values.
    pub fn get_accounts(&self) -> Result<Accounts> {
        Ok(
            match self.store.get("accounts") {
                Some(val) => { serde_json::from_value::<Accounts>(val)? },
                _ => {
                    Accounts {
                        last: None,
                        accounts: Vec::new()
                    }
                },
            }
        )
    }

    /// Save the new [Accounts] object's values in the store.
    ///
    /// # Arguments:
    /// * `accounts`: The [Accounts] object you wish to store.
    fn save_accounts(&self, accounts: &Accounts) -> Result<()> {
        self.store.set("accounts", serde_json::to_value(accounts)?);
        self.store.save()?;
        Ok(())
    }

    /// Add a new account to the store and save it.
    ///
    /// # Arguments
    /// * `user_id`: The Matrix account's userid in the format of @username:homeserver.
    ///
    /// This will set the `last` value to the user, and add the account to the `accounts` vector at index 0.
    ///
    /// If the account already exists, don't add a new duplicate account, just move it to index 0.
    pub fn add_account(&self, user_id: &String) -> Result<()> {
        let mut accounts = self.get_accounts()?;

        accounts.accounts.retain(|x| x != user_id);
        accounts.accounts.insert(0, user_id.to_string());
        accounts.last = Some(user_id.to_string());
        self.save_accounts(&accounts)
    }

    /// Remove an account from the store
    ///
    /// # Arguments:
    /// * `user_id`: The Matrix account's userid in the format of @username:homeserver.
    ///
    /// If the account does not exist, don't edit the store.
    ///
    /// If the account is also set to the `last` value in store, replace it with the first user in the `accounts` vector, if there is no other user, then last will be set to [None].
    pub fn remove_account(&self, user_id: String) -> Result<()> {
        let mut accounts = self.get_accounts()?;

        let before = accounts.accounts.len();
        accounts.accounts.retain(|x| x != &user_id);

        if accounts.accounts.len() == before {
            println!("{user_id} Not found");
            return Ok(())
        }

        if accounts.last == Some(user_id) {
            accounts.last = accounts.accounts.first().cloned();
        }

        self.save_accounts(&accounts)
    }

    /// Return the userid of the account who was added to the store.
    pub fn get_last(&self) -> Result<Option<String>> {
        Ok(self.get_accounts()?.last)
    }
}