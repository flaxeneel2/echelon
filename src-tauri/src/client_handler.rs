use matrix_sdk::{
    ruma::api::client::account::register::v3::Request as RegistrationRequest,
    Client
};
use ruma::api::client::uiaa::{AuthData, RegistrationToken};

pub struct ClientHandler {
    matrix_client: Client
}

impl ClientHandler {
    pub async fn register(
        username: String,
        password: String,
        homeserver: String,
        registration_token: Option<String>
    ) -> anyhow::Result<ClientHandler> {
        let client = Client::builder()
            .homeserver_url(homeserver)
            .build()
            .await
            .expect("Failed to create Matrix client");

        println!("Registration token: {:?}", registration_token);

        let mut registration_request = RegistrationRequest::new();
        registration_request.username = Some(username.clone());
        registration_request.password = Some(password.clone());
        if let Some(token) = registration_token.clone() {
            registration_request.auth = Some(AuthData::RegistrationToken(
                RegistrationToken::new(token)
            ));
        }

        println!("auth token thting {:?}", registration_request);

        let auth = client.matrix_auth();

        let reg_builder = auth.register(registration_request.clone());
        match reg_builder.await {
            Ok(res) => {
                println!("Registration worked immediately (no challenge-response), ID is {}", res.user_id);
                Ok(
                    ClientHandler {
                        matrix_client: client
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
                                    matrix_client: client
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
}