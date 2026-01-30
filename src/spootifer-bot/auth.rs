use crate::{
    db::{AuthRequest, OAuthToken},
    spotify::init_spotify,
    tidal::init_tidal,
};
use chrono::Utc;
use prawn::client::TidalClient;
use rspotify::clients::BaseClient;
use rspotify::{AuthCodeSpotify, ClientError, prelude::OAuthClient};
use serde::ser::StdError;
use std::fmt::{Display, Formatter};
use std::{error::Error, fmt::Debug};

#[derive(Debug)]
pub struct AuthError {
    msg: String,
}

impl Display for AuthError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "error during auth: {}", self.msg)
    }
}
impl Error for AuthError {}

impl From<Box<dyn Error>> for AuthError {
    fn from(value: Box<dyn Error>) -> Self {
        return AuthError {
            msg: value.to_string(),
        };
    }
}

impl From<ClientError> for AuthError {
    fn from(value: ClientError) -> Self {
        return AuthError {
            msg: value.to_string(),
        };
    }
}
impl From<Box<dyn StdError + std::marker::Send + Sync>> for AuthError {
    fn from(value: Box<dyn StdError + std::marker::Send + Sync>) -> Self {
        return AuthError {
            msg: value.to_string(),
        };
    }
}

pub trait ExchangeToken {
    async fn exchange_token(
        auth_request: AuthRequest,
        code: String,
        user_id: i64,
    ) -> Result<OAuthToken, AuthError>;
}

impl ExchangeToken for AuthCodeSpotify {
    async fn exchange_token(
        _: AuthRequest,
        code: String,
        user_id: i64,
    ) -> Result<OAuthToken, AuthError> {
        let client = match init_spotify() {
            Ok(c) => c,
            Err(e) => return Err(e.into()),
        };

        match client.request_token(code.as_str()).await {
            Ok(_) => {}
            Err(e) => return Err(e.into()),
        };

        let maybe_token = client.get_token();

        let maybe_token = match maybe_token.lock().await {
            Ok(t) => t,
            Err(e) => {
                return Err(AuthError {
                    msg: format!("{:?}", e),
                });
            }
        };

        let token = match maybe_token.clone() {
            Some(t) => t,
            None => {
                return Err(AuthError {
                    msg: String::from("failed to get token"),
                });
            }
        };

        token.into_oauth_token(user_id).ok_or(AuthError {
            msg: String::from("failed to get oauth token from token"),
        })
    }
}

impl ExchangeToken for TidalClient {
    async fn exchange_token(
        auth_request: AuthRequest,
        code: String,
        user_id: i64,
    ) -> Result<OAuthToken, AuthError> {
        let client = init_tidal()?;

        let Some(stored_verifier) = auth_request.pkce_code_verifier else {
            return Err(AuthError {
                msg: String::from("auth request missing pkce code verifier"),
            });
        };

        let token = match client.exchange_code_for_token(stored_verifier, code).await {
            Ok(t) => t,
            Err(e) => return Err(AuthError { msg: e.to_string() }),
        };

        token.into_oauth_token(user_id).ok_or(AuthError {
            msg: String::from("failed to get oauth token from token"),
        })
    }
}

pub trait IntoOAuthToken {
    fn into_oauth_token(&self, user_id: i64) -> Option<OAuthToken>;
}

impl IntoOAuthToken for prawn::client::Token {
    fn into_oauth_token(&self, user_id: i64) -> Option<OAuthToken> {
        Some(OAuthToken {
            user_id,
            refresh_token: self.refresh_token.clone(),
            access_token: self.access_token.clone(),
            expiry_time: self.expiry.clone(),
            token_type: String::from("Bearer"),
            deleted_at: None,
            created_at: Utc::now().to_rfc3339(),
            updated_at: Utc::now().to_rfc3339(),
            for_service: String::from("tidal"),
        })
    }
}

impl IntoOAuthToken for rspotify::Token {
    fn into_oauth_token(&self, user_id: i64) -> Option<OAuthToken> {
        Some(OAuthToken {
            user_id,
            refresh_token: self.refresh_token.clone(),
            access_token: self.access_token.clone(),
            expiry_time: self.expires_at?.to_string(),
            token_type: String::from("Bearer"),
            deleted_at: None,
            created_at: Utc::now().to_string(),
            updated_at: Utc::now().to_string(),
            for_service: "spotify".to_string(),
        })
    }
}
