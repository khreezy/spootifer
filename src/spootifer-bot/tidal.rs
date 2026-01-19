use crate::error;
use regex::Regex;
use rsgentidal::apis::Api;
use rsgentidal::client::{OAuthConfig, TidalClient, TidalClientConfig, Token};
use std::env;
use std::error::Error;
use std::fmt::{Display, Formatter};

static TIDAL_DOMAIN: &str = "tidal.com";
static TIDAL_ALBUM_LINK: &str = "https://tidal.com/album";

type Result<T> = std::result::Result<T, Box<dyn Error + Send + Sync>>;

#[derive(Debug, Clone)]
pub struct TidalError {
    msg: String,
    cause: String,
}

impl Display for TidalError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "error ")
    }
}

impl Error for TidalError {}

pub(crate) fn init_tidal() -> Result<TidalClient> {
    let client_id = env::var("TIDAL_CLIENT_ID")?;

    let redirect_uri = get_redirect_uri()?;

    let config = TidalClientConfig {
        oauth_config: OAuthConfig {
            redirect_uri: redirect_uri,
            client_id: client_id,
            client_secret: None,
        },
        auth_token: None,
    };

    match rsgentidal::client::TidalClient::new(config) {
        Ok(c) => Ok(c),
        Err(e) => Err(TidalError {
            msg: String::from("failed to initialize tidal client"),
            cause: e.to_string(),
        }
        .into()),
    }
}

pub(crate) fn init_tidal_with_token(token: Token) -> Result<TidalClient> {
    let client_id = env::var("TIDAL_CLIENT_ID")?;

    let redirect_uri = get_redirect_uri()?;

    let config = TidalClientConfig {
        oauth_config: OAuthConfig {
            redirect_uri: redirect_uri,
            client_id: client_id,
            client_secret: None,
        },
        auth_token: Some(token),
    };

    match rsgentidal::client::TidalClient::new(config) {
        Ok(c) => Ok(c),
        Err(e) => Err(TidalError {
            msg: String::from("failed to initialize tidal client"),
            cause: e.to_string(),
        }
        .into()),
    }
}

pub static DEFAULT_SCOPES: &'static [&'static str] = &[
    "user.read collection.read",
    "playlists.write",
    "collection.write",
    "playlists.read",
    "entitlements.read",
    "recommendations.read",
    "playback",
];

pub(crate) fn tidal_link_regex() -> Regex {
    Regex::new(
        r"https://tidal\.com/playlist/([0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12})",
    ).expect("failed to compile tidal regex")
}

pub(crate) fn get_redirect_uri() -> Result<String> {
    let base_uri = env::var("BASE_REDIRECT_URI")?;

    Ok(format!("{base_uri}/callback"))
}

pub(crate) fn contains_tidal_link(msg: String) -> bool {
    msg.contains(TIDAL_DOMAIN)
}

pub(crate) fn extract_playlist_id(link: String) -> Option<String> {
    let re = Regex::new(
        r"https://tidal\.com/playlist/([0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12})",
    )
    .expect("unable to compile regex");

    Some(re.captures(link.as_str())?.get(1)?.as_str().to_string())
}

pub(crate) fn extract_ids(link: &str) -> Vec<String> {
    let re = match Regex::new(
        r"(((?:https?://tidal\.com/track/|https?://tidal\.com/album/)([a-zA-Z0-9]+)))/u",
    ) {
        Ok(re) => re,
        Err(e) => {
            error!("Failed to compile regex: {}", e);
            return vec![];
        }
    };

    let matches = re.captures_iter(link);

    return matches
        .filter_map(|m| -> Option<Vec<String>> {
            if m.len() > 1 {
                let link = m.get(1)?.as_str();

                if is_album(link) {
                    Some(vec![m.get(3)?.as_str().to_string()])
                } else {
                    Some(vec![m.get(3)?.as_str().to_string()])
                }
            } else {
                None
            }
        })
        .flat_map(|x| x)
        .collect();
}

pub(crate) fn is_album(link: &str) -> bool {
    link.contains(TIDAL_ALBUM_LINK)
}

pub(crate) async fn get_album_track_ids(
    client: &TidalClient,
    album_id: String,
) -> Result<Vec<String>> {
    let album_tracks_resp = client
        .albums_api()
        .get_album_items(album_id.as_str(), None, None, None, None)
        .await?;

    let Some(album_track_data) = album_tracks_resp.data else {
        return Err(TidalError {
            msg: String::from("failed to get album track ids"),
            cause: String::from("response missing track  data"),
        }
        .into());
    };

    Ok(album_track_data
        .into_iter()
        .map(|t: rsgentidal::models::AlbumsItemsResourceIdentifier| -> String { t.id })
        .collect())
}
pub(crate) async fn get_track_ids(
    client: &TidalClient,
    tidal_ids: &Vec<String>,
) -> Result<Vec<String>> {
    let mut track_ids = vec![];

    for id in tidal_ids {
        match &mut get_album_track_ids(client, id.clone()).await {
            Ok(v) => track_ids.append(v),
            Err(_) => track_ids.push(id.clone()),
        }
    }

    Ok(track_ids)
}
