use chrono::Utc;
use std::env;
use std::error::Error;
use std::fmt::{Display, Formatter};
use tidalrs::{AuthzToken, TidalClient};

use crate::db::{IntoOAuthToken, OAuthToken};

type Result<T> = std::result::Result<T, Box<dyn Error + Send + Sync>>;

#[derive(Debug, Clone)]
pub struct TidalError;

impl Display for TidalError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "error ")
    }
}

impl Error for TidalError {}

pub(crate) fn init_tidal() -> Result<TidalClient> {
    let client_id = env::var("TIDAL_CLIENT_ID")?;

    Ok(TidalClient::new(client_id))
}

pub static DEFAULT_SCOPES: &str =
    "user.read collection.read playlists.write collection.write playlists.read";

impl IntoOAuthToken for AuthzToken {
    fn into_oauth_token(&self, user_id: i64) -> Option<OAuthToken> {
        Some(OAuthToken {
            user_id,
            refresh_token: self.refresh_token.clone()?,
            access_token: self.access_token.clone(),
            expiry_time: self.expires_in.to_string(),
            token_type: String::from("Bearer"),
            deleted_at: None,
            created_at: Utc::now().to_string(),
            updated_at: Utc::now().to_string(),
            for_service: "tidal".to_string(),
        })
    }
}

pub(crate) fn get_redirect_uri() -> Result<String> {
    let base_uri = env::var("BASE_REDIRECT_URI")?;

    Ok(format!("{base_uri}/callback"))
}
