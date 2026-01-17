use crate::{
    db::{AuthRequest, IntoOAuthToken, OAuthToken},
    spotify::init_spotify,
    tidal::{get_redirect_uri, init_tidal, DEFAULT_SCOPES},
};
use rspotify::clients::BaseClient;
use rspotify::{prelude::OAuthClient, AuthCodeSpotify, ClientError};
use serde::ser::StdError;
use std::fmt::{Display, Formatter};
use std::{error::Error, fmt::Debug};
use tidalrs::TidalClient;

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
                })
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
        let redirect_uri = get_redirect_uri()?;

        let client = init_tidal()?;
        let token = match client
            .pkce_authorize(
                code.as_str(),
                redirect_uri.as_str(),
                auth_request
                    .pkce_code_verifier
                    .ok_or(AuthError {
                        msg: String::from("missing code verifier on auth request"),
                    })?
                    .as_str(),
                DEFAULT_SCOPES,
            )
            .await
        {
            Ok(t) => t,
            Err(e) => return Err(AuthError { msg: e.to_string() }),
        };

        token.into_oauth_token(user_id).ok_or(AuthError {
            msg: String::from("failed to get oauth token from token"),
        })
    }
}
