use crate::error;
use crate::spotify::SpotifyResource;
use log::info;
use prawn::apis::Api;
use prawn::client::{OAuthConfig, TidalClient, TidalClientConfig, TidalClientError, Token};
use prawn::models::IncludedInner;
use regex::Regex;
use std::env;
use std::error::Error;
use std::fmt::{Display, Formatter};
use std::sync::Arc;
use std::thread::sleep;
use std::time::Duration;

static TIDAL_DOMAIN: &str = "tidal.com";
static TIDAL_ALBUM_LINK: &str = "https://tidal.com/album";

type Result<T> = std::result::Result<T, Box<dyn Error + Send + Sync>>;

#[derive(Debug, Clone)]
pub enum TidalError {
    ClientInitializationError { cause: String },
    ApiError { api: String, cause: String },
}

impl Display for TidalError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::ClientInitializationError { cause } => {
                write!(f, "failed to initialize client: {}", cause)
            }
            Self::ApiError { api, cause } => write!(f, "failed call {} api: {}", api, cause),
        }
    }
}

impl Error for TidalError {}

impl From<TidalClientError> for TidalError {
    fn from(value: TidalClientError) -> Self {
        Self::ClientInitializationError {
            cause: value.to_string(),
        }
    }
}

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

    Ok(prawn::client::TidalClient::new(config)?)
}

pub(crate) async fn init_tidal_with_secret() -> Result<TidalClient> {
    let client_id = env::var("TIDAL_CLIENT_ID")?;
    let client_secret = env::var("TIDAL_CLIENT_SECRET")?;

    let redirect_uri = get_redirect_uri()?;

    let config = TidalClientConfig {
        oauth_config: OAuthConfig {
            redirect_uri: redirect_uri,
            client_id: client_id,
            client_secret: Some(client_secret),
        },
        auth_token: None,
    };

    let client = prawn::client::TidalClient::new(config)?;

    let token = client
        .exchange_client_credentials_for_token(DEFAULT_SCOPES.to_vec())
        .await?;

    Ok(client.with_token(token)?)
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

    Ok(prawn::client::TidalClient::new(config)?)
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
    let mut track_ids: Vec<String> = vec![];
    let album_tracks = client
        .albums_api()
        .get_album_items(album_id.as_str(), None, None, None, None)
        .await?;

    let Some(album_tracks_data) = album_tracks.data else {
        return Err(TidalError::ApiError {
            api: String::from("album_track_ids"),
            cause: String::from("track data missing"),
        }
        .into());
    };

    for track_id in album_tracks_data {
        track_ids.push(track_id.id)
    }

    let mut maybe_next = album_tracks.links.meta;
    while let Some(next) = maybe_next.clone() {
        let album_tracks = client
            .albums_api()
            .get_album_items(album_id.as_str(), Some(&next.next_cursor), None, None, None)
            .await?;
        let Some(album_tracks_data) = album_tracks.data else {
            return Err(TidalError::ApiError {
                api: String::from("album_track_ids"),
                cause: String::from("track data missing"),
            }
            .into());
        };

        for track_id in album_tracks_data {
            track_ids.push(track_id.id)
        }

        sleep(Duration::from_millis(200));
        maybe_next = album_tracks.links.meta
    }

    Ok(track_ids)
}
pub(crate) async fn get_track_ids(
    client: &TidalClient,
    tidal_ids: &Vec<String>,
) -> Result<Vec<String>> {
    let mut track_ids = vec![];

    for id in tidal_ids {
        match &mut get_album_track_ids(client, id.clone()).await {
            Ok(v) => track_ids.append(v),
            Err(e) => {
                error!("failed to fetch album ids: {}", e);
                track_ids.push(id.clone())
            }
        }
    }

    Ok(track_ids)
}

pub(crate) enum TidalResourceType {
    Album,
    Track,
}

pub(crate) async fn get_tidal_ids_from_spotify_resources(
    client: Arc<TidalClient>,
    spotify_resources: &Vec<SpotifyResource>,
) -> Result<Vec<String>> {
    let mut ids = vec![];

    for resource in spotify_resources {
        let search_string = resource.title.to_string() + " " + resource.artist.as_str();
        info!("searching for {}", search_string);

        let id = match resource.tidal_resource_type {
            TidalResourceType::Album => {
                let search = client
                    .search_results_api()
                    .get_search_result_albums(
                        search_string.as_str(),
                        None,
                        None,
                        None,
                        Some(vec![String::from("albums")]),
                    )
                    .await?;

                let top_albums = search.included.ok_or(TidalError::ApiError {
                    api: "search".to_string(),
                    cause: "relationships missing".to_string(),
                })?;

                let Some(IncludedInner::Albums(top_album)) = top_albums.iter().find(|i| -> bool {
                    match i {
                        IncludedInner::Albums(a) => {
                            let Some(attrs) = a.attributes.clone() else {
                                return false;
                            };
                            attrs.title == resource.title
                        }
                        _ => false,
                    }
                }) else {
                    error!("no album results");
                    continue;
                };

                top_album.id.clone()
            }
            TidalResourceType::Track => {
                let search = client
                    .search_results_api()
                    .get_search_result_tracks(
                        search_string.as_str(),
                        None,
                        None,
                        None,
                        Some(vec![String::from("tracks")]),
                    )
                    .await?;

                let top_tracks = search.included.ok_or(TidalError::ApiError {
                    api: "search".to_string(),
                    cause: "relationships missing".to_string(),
                })?;

                let Some(IncludedInner::Tracks(top_track)) = top_tracks.iter().find(|i| -> bool {
                    match i {
                        IncludedInner::Tracks(t) => {
                            let Some(attrs) = t.attributes.clone() else {
                                return false;
                            };
                            attrs.title == resource.title
                        }
                        _ => false,
                    }
                }) else {
                    error!("no track results");
                    continue;
                };

                top_track.id.clone()
            }
        };

        sleep(Duration::from_millis(200));
        ids.push(id.to_string())
    }

    Ok(ids)
}
