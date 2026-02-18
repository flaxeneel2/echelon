use crate::events::client_events::ClientEvents;
use crate::sync_manager::SyncManager;
use matrix_sdk::{ruma::api::client::account::register::v3::Request as RegistrationRequest, AuthSession, Client, SessionMeta, SessionTokens};
use ruma::api::client::uiaa::{AuthData, RegistrationToken};
use std::path::Path;
use matrix_sdk::authentication::matrix::MatrixSession;
use tauri::{AppHandle, Manager};

pub struct ClientHandler {
    matrix_client: Client,
    pub sync_manager: SyncManager,
    app_handle: AppHandle,
}

impl ClientHandler {
    pub async fn new(app_handle: AppHandle) -> Self {
        ClientHandler {
            matrix_client: Client::new("https://matrix.org".parse().unwrap()).await.expect("Failed to create Matrix client"),
            sync_manager: SyncManager::new(),
            app_handle,
        }
    }

    pub fn get_client(&self) -> &Client {
        &self.matrix_client
    }

    pub async fn register(
        &self,
        username: String,
        password: String,
        homeserver: String,
        registration_token: Option<String>
    ) -> anyhow::Result<ClientHandler> {
        let client: Client = self.get_new_client(&username, homeserver).await?;
        println!("Registration token: {:?}", registration_token);

        let mut registration_request = RegistrationRequest::new();
        registration_request.username = Some(username.clone());
        registration_request.password = Some(password.clone());
        if let Some(token) = registration_token.clone() {
            registration_request.auth = Some(AuthData::RegistrationToken(
                RegistrationToken::new(token)
            ));
        }

        println!("auth token thing {:?}", registration_request);

        let auth = client.matrix_auth();

        let reg_builder = auth.register(registration_request.clone());
        match reg_builder.await {
            Ok(res) => {
                println!("Registration worked immediately (no challenge-response), ID is {}", res.user_id);
                Ok(
                    ClientHandler {
                        matrix_client: client,
                        sync_manager: SyncManager::new(),
                        app_handle: self.app_handle.clone(),
                    }
                )
            },
            Err(e) => {
                println!("Registration failed, trying challenge-response");
                if let Some(uiaa_info) = e.as_uiaa_response() {
                    let session = uiaa_info.session.clone();
                    println!("Received UIAA response with session: {:?}", session);

                    let mut reg_token = RegistrationToken::new(registration_token.unwrap());
                    reg_token.session = session;
                    let auth_data = AuthData::RegistrationToken(
                        reg_token
                    );
                    registration_request.auth = Some(auth_data);
                    let final_response = client.matrix_auth().register(registration_request).await;
                    match final_response {
                        Ok(res) => {
                            println!("Registration successful after challenge-response, ID is {}", res.user_id);
                            Ok(
                                ClientHandler {
                                    matrix_client: client,
                                    sync_manager: SyncManager::new(),
                                    app_handle: self.app_handle.clone(),
                                }
                            )
                        },
                        Err(e) => {
                            println!("Registration failed after challenge-response: {:?}", e);
                            Err(anyhow::anyhow!("Registration failed after challenge-response: {:?}", e))
                        }
                    }
                } else {
                    println!("Registration failed with error: {:?}", e);
                    Err(anyhow::anyhow!("Registration failed: {:?}", e))
                }
            }
        }
    }

    async fn get_new_client(&self, username: &String, new_homeserver: String) -> anyhow::Result<Client> {
        Ok(
            Client::builder()
                .homeserver_url(new_homeserver)
                .sqlite_store(Path::join(&self.app_handle.path().app_data_dir()?, format!("{}_data", username)), None)
                .build()
                .await?
        )
    }

    pub async fn login(
        &self,
        username: String,
        password: String,
        homeserver: String
    ) -> anyhow::Result<Option<ClientHandler>> {
        let new_client = self.get_new_client(&username, homeserver).await?;
        new_client.matrix_auth().login_username(&username, &password).send().await?;

        ClientEvents::register_events(&new_client, self.app_handle.clone());
        
        Ok(Some(ClientHandler {
            matrix_client: new_client,
            sync_manager: SyncManager::new(),
            app_handle: self.app_handle.clone(),
        }))
    }

}