use crate::account::account_reset_types::AccountResetType;
use crate::events::client_events::ClientEvents;
use crate::sync_manager::SyncManager;
use crate::SecretState;
use crate::store::EchelonStore;
use matrix_sdk::authentication::matrix::MatrixSession;
use matrix_sdk::authentication::oauth::registration::{
    ApplicationType, ClientMetadata, Localized, OAuthGrantType,
};
use matrix_sdk::authentication::oauth::UrlOrQuery;
use matrix_sdk::encryption::CrossSigningResetAuthType;
use matrix_sdk::utils::local_server::LocalServerBuilder;
use matrix_sdk::{
    ruma::api::client::account::register::v3::Request as RegistrationRequest, AuthSession, Client,
    SessionMeta, SessionTokens,
};
use ruma::api::client::uiaa::{AuthData, Password, RegistrationToken, UserIdentifier};
use ruma::serde::Raw;
use ruma::{OwnedDeviceId, OwnedUserId};
use std::path::Path;
use tauri::{AppHandle, Manager, Url};
use tauri_plugin_opener::OpenerExt;
use tracing::{debug, error};
use crate::secret::Session;

pub struct ClientHandler {
    matrix_client: Client,
    pub sync_manager: SyncManager,
    app_handle: AppHandle,
}

impl ClientHandler {
    pub async fn new(app_handle: AppHandle) -> Self {
        ClientHandler {
            matrix_client: Client::new("https://matrix.org".parse().unwrap())
                .await
                .expect("Failed to create Matrix client"),
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
        registration_token: Option<String>,
    ) -> anyhow::Result<ClientHandler> {
        let client: Client = self.get_new_client(&username, &homeserver, None).await?;

        let mut registration_request = RegistrationRequest::new();
        registration_request.username = Some(username.clone());
        registration_request.password = Some(password.clone());
        if let Some(token) = registration_token.clone() {
            registration_request.auth =
                Some(AuthData::RegistrationToken(RegistrationToken::new(token)));
        }

        let auth = client.matrix_auth();

        let reg_builder = auth.register(registration_request.clone());
        match reg_builder.await {
            Ok(res) => {
                debug!(
                    "Registration worked immediately (no challenge-response), ID is {}",
                    res.user_id
                );
                Ok(ClientHandler {
                    matrix_client: client,
                    sync_manager: SyncManager::new(),
                    app_handle: self.app_handle.clone(),
                })
            }
            Err(e) => {
                debug!("Registration failed, trying challenge-response");
                if let Some(uiaa_info) = e.as_uiaa_response() {
                    let session = uiaa_info.session.clone();
                    debug!("Received UIAA response with session: {:?}", session);

                    let mut reg_token = RegistrationToken::new(registration_token.unwrap());
                    reg_token.session = session;
                    let auth_data = AuthData::RegistrationToken(reg_token);
                    registration_request.auth = Some(auth_data);
                    let final_response = client.matrix_auth().register(registration_request).await;
                    match final_response {
                        Ok(res) => {
                            debug!(
                                "Registration successful after challenge-response, ID is {}",
                                res.user_id
                            );
                            Ok(ClientHandler {
                                matrix_client: client,
                                sync_manager: SyncManager::new(),
                                app_handle: self.app_handle.clone(),
                            })
                        }
                        Err(e) => {
                            error!("Registration failed after challenge-response: {:?}", e);
                            Err(anyhow::anyhow!(
                                "Registration failed after challenge-response: {:?}",
                                e
                            ))
                        }
                    }
                } else {
                    error!("Registration failed with error: {:?}", e);
                    Err(anyhow::anyhow!("Registration failed: {:?}", e))
                }
            }
        }
    }

    async fn get_new_client(
        &self,
        username: &String,
        new_homeserver: &String,
        sqlite_pwd: Option<String>,
    ) -> anyhow::Result<Client> {
        Ok(Client::builder()
            .homeserver_url(new_homeserver)
            .sqlite_store(
                Path::join(
                    &self.app_handle.path().app_data_dir()?,
                    format!(
                        "{}_{}_data",
                        username,
                        Url::parse(new_homeserver)?.domain().unwrap()
                    ),
                ),
                sqlite_pwd.as_deref(),
            )
            .build()
            .await?)
    }

    /// Log in a user with OAuth2 authentication using their homeserver
    ///
    /// # Arguments
    /// * `homeserver_url` - The URL of the homeserver to create a client for OAuth.
    async fn get_oauth_client(&self, new_homeserver: &String) -> anyhow::Result<Client> {
        let homeserver_url: Url = Url::parse(new_homeserver)?;
        let client = Client::new(homeserver_url).await?;
        Ok(client)
    }

    pub async fn login(
        &self,
        username: String,
        password: String,
        homeserver: String,
    ) -> anyhow::Result<Option<ClientHandler>> {
        // Derive the full Matrix user ID so we can look up / generate the sqlite password
        // before we even open the store, ensuring the DB is always encrypted from first open.
        let user_id = format!(
            "@{}:{}",
            username,
            Url::parse(&homeserver)?.domain().unwrap_or(&homeserver)
        );
        let secrets = self.app_handle.state::<SecretState>();
        let sqlite_pwd = secrets.0.get_or_create_sqlite_pwd(&user_id)?;

        let new_client = self.get_new_client(&username, &homeserver, Some(sqlite_pwd)).await?;
        new_client
            .matrix_auth()
            .login_username(&username, &password)
            .initial_device_display_name("Echelon")
            .send()
            .await?;

        ClientEvents::register_events(&new_client, self.app_handle.clone());

        // store the session tokens in stronghold
        let session_tokens = new_client.session_tokens().unwrap();
        secrets.0.set_session(&crate::secret::Session {
            user_id: new_client.user_id().unwrap().to_string(),
            device_id: new_client.device_id().map(|d| d.to_string()).unwrap_or_default(),
            access_token: session_tokens.access_token,
            refresh_token: session_tokens.refresh_token,
        })?;

        // store the new username
        let echelon_store = EchelonStore::new(&self.app_handle)?;
        echelon_store.add_account(&new_client.user_id().unwrap().to_string())?;

        Ok(Some(ClientHandler {
            matrix_client: new_client,
            sync_manager: SyncManager::new(),
            app_handle: self.app_handle.clone(),
        }))
    }

    /// Log in a user with OAuth2 authentication using their homeserver
    ///
    /// # Arguments
    /// * `homeserver` - The URL of the homeserver to log in to.
    /// * `login` - If true, the user has registered already so log them in, otherwise register
    pub async fn oauth_login(
        &self,
        homeserver: String,
        login: bool,
    ) -> anyhow::Result<Option<ClientHandler>> {
        // Create and generate an OAuth Handler
        let new_client = self.get_oauth_client(&homeserver).await?;
        let oauth = new_client.oauth();

        // Fetch metadata from homeserver to ensure that it supports OAuth
        // If it fails, it throws an exception to user.rs::oauth_login which passes it to front-end
        oauth.server_metadata().await?;

        // Make a listener to listen on a random port to receive the GET request
        let (redirect_uri, redirect_handle) = LocalServerBuilder::new().spawn().await?;

        // If user has registered and is logging in
        if login {
            // oauth.restore_registered_client()
        }
        // If the user hasn't registered, we register them
        else {
            // Setup client metadata
            let url = Url::parse("https://github.com/flaxeneel2/echelon/")?;
            let new_client_url = Localized::new(url, Vec::new());
            let grant_types: Vec<OAuthGrantType> = vec![
                OAuthGrantType::AuthorizationCode {
                    redirect_uris: vec![redirect_uri.clone()],
                },
                OAuthGrantType::DeviceCode,
            ];
            let client_metadata =
                ClientMetadata::new(ApplicationType::Native, grant_types, new_client_url);
            let raw_client_metadata = Raw::new(&client_metadata)?;
            oauth.register_client(&raw_client_metadata).await?;
        }
        // Build authorization data and login, then build the OAuthAuthCodeUrlBuilder
        let auth_data = oauth
            .login(redirect_uri.clone(), None, None, None)
            .build()
            .await?;
        self.app_handle
            .opener()
            .open_url(auth_data.url, None::<&str>)?;

        // Wait for redirect
        let query = redirect_handle
            .await
            .ok_or_else(|| anyhow::anyhow!("OAuth redirect was cancelled or timed out"))?;

        // Finish Login, the SDK verifies the csrf token internally
        oauth
            .finish_login(UrlOrQuery::Query(query.to_string()))
            .await?;

        // store the session tokens in stronghold
        let session_tokens = new_client.session_tokens().unwrap();
        let secrets = self.app_handle.state::<SecretState>();
        secrets.0.set_session(&crate::secret::Session {
            user_id: new_client.user_id().unwrap().to_string(),
            device_id: new_client.device_id().map(|d| d.to_string()).unwrap_or_default(),
            access_token: session_tokens.access_token,
            refresh_token: session_tokens.refresh_token,
        })?;

        // store the new username
        let echelon_store = EchelonStore::new(&self.app_handle)?;
        echelon_store.add_account(&new_client.user_id().unwrap().to_string())?;

        ClientEvents::register_events(&new_client, self.app_handle.clone());

        Ok(Some(ClientHandler {
            matrix_client: new_client,
            sync_manager: SyncManager::new(),
            app_handle: self.app_handle.clone(),
        }))
    }

    pub async fn restore_session(
        &self,
        username: String,
        homeserver: String,
    ) -> anyhow::Result<Option<ClientHandler>> {
        let user_id = format!(
            "@{}:{}",
            username,
            Url::parse(&homeserver)?.domain().unwrap_or(&homeserver)
        );
        let secrets = self.app_handle.state::<SecretState>();
        let sqlite_pwd = secrets.0.get_sqlite_pwd(&user_id)?;

        let new_client = self.get_new_client(&username, &homeserver, sqlite_pwd).await?;
        match secrets.0.get_session(&*user_id) {
            Ok(session_opt) => {
                match session_opt {
                    None => {
                        error!("No session found for user, cannot restore");
                    }
                    Some(session) => {
                        new_client
                            .restore_session(AuthSession::Matrix(MatrixSession {
                                meta: SessionMeta {
                                    user_id: OwnedUserId::try_from(session.user_id)?,
                                    device_id: OwnedDeviceId::try_from(session.device_id)?,
                                },
                                tokens: SessionTokens {
                                    access_token: session.access_token,
                                    refresh_token: session.refresh_token,
                                }
                            }))
                            .await?
                    }
                }
            }
            Err(e) => {
                error!("Some other error on trying to make session: {e}");
            }
        }


        ClientEvents::register_events(&new_client, self.app_handle.clone());

        Ok(Some(ClientHandler {
            matrix_client: new_client,
            sync_manager: SyncManager::new(),
            app_handle: self.app_handle.clone(),
        }))
    }

    pub async fn reset_account(
        &self,
        account_reset_type: AccountResetType,
        password: Option<String>,
        key_backup: Option<String>,
    ) -> anyhow::Result<()> {
        let client = &self.matrix_client;
        let recovery = client.encryption().recovery();

        match account_reset_type {
            AccountResetType::IdentityReset => {
                debug!("Starting identity reset...");
                if let Some(handle) = recovery.reset_identity().await? {
                    match handle.auth_type() {
                        CrossSigningResetAuthType::Uiaa(uiaa_info) => {
                            debug!("UIAA authentication required for identity reset");
                            if let Some(pwd) = password {
                                // Create password authentication data
                                let user_id = client
                                    .user_id()
                                    .ok_or_else(|| anyhow::anyhow!("No user ID available"))?;

                                let mut password_auth = Password::new(
                                    UserIdentifier::UserIdOrLocalpart(user_id.to_string()),
                                    pwd,
                                );

                                // Set the session if available
                                if let Some(session) = &uiaa_info.session {
                                    password_auth.session = Some(session.clone());
                                }

                                // Perform the reset with password authentication
                                handle
                                    .reset(Some(AuthData::Password(password_auth)))
                                    .await?;
                                debug!("Identity reset completed successfully");
                            } else {
                                return Err(anyhow::anyhow!(
                                    "Password required for UIAA authentication"
                                ));
                            }
                        }
                        CrossSigningResetAuthType::OAuth(oauth_info) => {
                            debug!("OAuth authentication required: {:?}", oauth_info);
                            // For OAuth, the user needs to complete authentication via browser
                            // This typically requires opening a browser and completing the OAuth flow
                            handle.reset(None).await?;
                            debug!("Identity reset initiated with OAuth");
                        }
                    }
                }
            }
            AccountResetType::KeyBackupReset => {
                debug!("Starting key backup reset...");

                if let Some(key_backup) = key_backup {
                    recovery.recover(&key_backup).await?;
                } else {
                    Err(anyhow::anyhow!(
                        "KeyBackup reset required for key backup reset"
                    ))?;
                }
            }
        }

        Ok(())
    }
}