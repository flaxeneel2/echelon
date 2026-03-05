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

    pub fn get_accounts(&self) -> Result<Accounts> {
        let accounts = match self.store.get("accounts") {
            Some(val) => { serde_json::from_value::<Accounts>(val)? },
            _ => {
                Accounts {
                    last: None,
                    accounts: Vec::new()
                }
            },
        };

        Ok(accounts)
    }

    fn save_accounts(&self, accounts: &Accounts) -> Result<()> {
        self.store.set("accounts", serde_json::to_value(accounts)?);
        self.store.save()?;
        Ok(())
    }

    pub fn add_account(&self, user_id: &String) -> Result<()> {
        let mut accounts = self.get_accounts()?;

        accounts.accounts.retain(|x| x != user_id);
        accounts.accounts.insert(0, user_id.to_string());
        accounts.last = Some(user_id.to_string());
        self.save_accounts(&accounts)
    }

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

    pub fn get_last(&self) -> Result<Option<String>> {
        Ok(self.get_accounts()?.last)
    }
}