use log::error;
use ordermap::OrderSet;
use prawn::client::TidalClient;
use regex::Regex;
use rspotify::clients::BaseClient;
use rspotify::model::{AlbumId, FullAlbum, FullTrack, Image, PlayableId, TrackId};
use rspotify::{AuthCodeSpotify, ClientCredsSpotify, Config, Credentials, OAuth, Token, scopes};
use std::env;
use std::error::Error;
use std::fmt::{Display, Formatter};
use std::sync::Arc;

use crate::discord::ServiceResources;
use crate::tidal;

const SPOTIFY_DOMAIN: &str = "open.spotify.com";
const SPOTIFY_SHORTENED_DOMAIN: &str = "spotify.link";
const MAX_REDIRECT_DEPTH: u32 = 5;

const SPOTIFY_ALBUM_LINK: &str = "https://open.spotify.com/album/";

type Result<T> = std::result::Result<T, Box<dyn Error + Send + Sync>>;

pub(crate) fn is_album(link: &str) -> bool {
    link.contains(SPOTIFY_ALBUM_LINK)
}

pub(crate) fn contains_spotify_link(msg: &str) -> bool {
    msg.contains(SPOTIFY_DOMAIN)
}

#[derive(Debug, Clone)]
pub(crate) enum IdType {
    Track(String),
    Album(String),
}

pub(crate) fn extract_ids(link: &str) -> Vec<IdType> {
    let re = match Regex::new(
        r"(((?:https?://open\.spotify\.com/track/|https?://open\.spotify\.com/album/|spotify:track:|spotify:album:)([a-zA-Z0-9]+))|https?://spotify.link/[a-zA-Z0-9]+)",
    ) {
        Ok(re) => re,
        Err(e) => {
            error!("Failed to compile regex: {}", e);
            return vec![];
        }
    };

    let matches = re.captures_iter(link);

    return matches
        .filter_map(|m| -> Option<Vec<IdType>> {
            if m.len() > 1 {
                let link = m.get(1)?.as_str();

                return if link.contains(SPOTIFY_SHORTENED_DOMAIN) {
                    let full_url: String = match expand_spotify_short_link(link, 0) {
                        Ok(url) => url,
                        Err(_) => return None,
                    };

                    Some(extract_ids(&full_url))
                } else if is_album(link) {
                    Some(vec![IdType::Album(m.get(3)?.as_str().to_string())])
                } else {
                    Some(vec![IdType::Track(m.get(3)?.as_str().to_string())])
                };
            } else {
                None
            }
        })
        .flat_map(|x| x)
        .collect();
}

#[derive(Debug, Clone)]
pub struct SpotifyErr;

impl Display for SpotifyErr {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "error ")
    }
}

impl Error for SpotifyErr {}

fn expand_spotify_short_link<'a>(link: &str, depth: u32) -> Result<String> {
    if depth >= MAX_REDIRECT_DEPTH {
        return Ok(link.to_string());
    }

    let result = reqwest::blocking::Client::new()
        .get(link)
        .header("User-Agent", "python-requests/2.31.0")
        .header("Accept-Encoding", "gzip, deflate")
        .header("Accept", "*/*")
        .header("Connection", "keep-alive")
        .send()?;

    let expanded_url = result.url().as_str();

    if expanded_url.contains(SPOTIFY_DOMAIN) {
        return expand_spotify_short_link(expanded_url, depth);
    }

    Ok(expanded_url.to_string())
}

pub(crate) fn init_spotify_from_token(token: Token) -> Result<AuthCodeSpotify> {
    let config = Config {
        ..Default::default()
    };

    // Please notice that protocol of redirect_uri, make sure it's http (or
    // https). It will fail if you mix them up.
    let oauth = OAuth {
        scopes: scopes!("playlist-modify-public"),
        redirect_uri: env::var("SPOTIFY_REDIRECT_URI")
            .unwrap_or_else(|_| SPOTIFY_DOMAIN.to_string()),
        ..Default::default()
    };

    let creds = match Credentials::from_env() {
        Some(creds) => creds,
        None => {
            error!("Spotify credentials not set");
            return Err(SpotifyErr.into());
        }
    };

    Ok(AuthCodeSpotify::from_token_with_config(
        token, creds, oauth, config,
    ))
}

pub(crate) fn init_spotify() -> Result<AuthCodeSpotify> {
    let config = Config {
        ..Default::default()
    };

    let base_redirect_uri = url::Url::parse(env::var("BASE_REDIRECT_URI")?.as_str())?;
    let redirect_uri = String::from(base_redirect_uri.join("/callback")?.as_str());

    let oauth = OAuth {
        scopes: scopes!("playlist-modify-public"),
        redirect_uri: redirect_uri,
        ..Default::default()
    };

    let creds = match Credentials::from_env() {
        Some(creds) => creds,
        None => {
            error!("Spotify credentials not set");
            return Err(SpotifyErr.into());
        }
    };

    Ok(AuthCodeSpotify::with_config(creds, oauth, config))
}

pub(crate) fn extract_playlist_id(link: String) -> Option<String> {
    let re = Regex::new(r"https://open\.spotify\.com/playlist/([a-zA-Z0-9]+)")
        .expect("unable to compile regex");

    Some(re.captures(link.as_str())?.get(1)?.as_str().to_string())
}

pub(crate) async fn get_album_cover_image_from_track(
    spotify: &ClientCredsSpotify,
    track_id: &str,
) -> Result<Option<Image>> {
    let track_id = TrackId::from_id(track_id)?;

    let track: FullTrack = spotify.track(track_id, None).await?;

    Ok(track.album.images.into_iter().next())
}

pub(crate) async fn get_album_cover_image(
    spotify: &ClientCredsSpotify,
    album_id: Option<&String>,
) -> Result<Option<Image>> {
    match album_id {
        None => return Ok(None),
        Some(id) => {
            let album_id = AlbumId::from_id(id)?;

            let album: FullAlbum = spotify.album(album_id, None).await?;

            Ok(album.images.into_iter().next())
        }
    }
}

pub(crate) async fn get_album_track_ids(
    client: &Arc<ClientCredsSpotify>,
    album_id: String,
) -> Vec<String> {
    let album_id = match AlbumId::from_id(album_id) {
        Ok(id) => id,
        Err(e) => {
            error!("Failed to get album id: {}", e.to_string());
            return vec![];
        }
    };

    let album = match client.album(album_id, None).await {
        Ok(a) => a,
        Err(e) => {
            error!("Failed to get album: {}", e.to_string());
            return vec![];
        }
    };

    album
        .tracks
        .items
        .into_iter()
        .filter_map(|t| -> Option<String> {
            match t.id {
                Some(id) => Some(id.to_string().replace("spotify:track:", "")),
                None => {
                    error!("couldn't get track id");
                    None
                }
            }
        })
        .collect()
}

pub(crate) async fn get_track_ids<'a>(
    client: &Arc<ClientCredsSpotify>,
    spotify_ids: &'a Vec<IdType>,
) -> Vec<PlayableId<'a>> {
    let mut track_ids = vec![];

    for id in spotify_ids {
        match id {
            IdType::Track(t) => {
                if let Ok(track_id) = TrackId::from_id(t) {
                    track_ids.push(PlayableId::from(track_id));
                }
            }
            IdType::Album(a) => {
                let raw_album_track_ids = get_album_track_ids(client, a.to_string()).await;
                track_ids.extend(
                    raw_album_track_ids
                        .into_iter()
                        .filter_map(|id| Some(PlayableId::from(TrackId::from_id(id).ok()?))),
                );
            }
        }
    }

    track_ids
}

pub(crate) async fn get_album_images(
    client: &Arc<ClientCredsSpotify>,
    spotify_ids: &Vec<IdType>,
) -> OrderSet<String> {
    let mut images = OrderSet::new();

    for id in spotify_ids {
        match id {
            IdType::Track(t) => {
                if let Ok(image) = get_album_cover_image_from_track(client, &t).await {
                    if let Some(img) = image {
                        images.insert(img.url);
                    }
                }
            }
            IdType::Album(a) => {
                if let Ok(image) = get_album_cover_image(client, Some(&a)).await {
                    if let Some(img) = image {
                        images.insert(img.url);
                    }
                }
            }
        }
    }

    images
}

pub enum SpotifyResource {
    Album(FullAlbum),
    Track(FullTrack),
}

pub async fn get_spotify_resources(
    client: &ClientCredsSpotify,
    spotify_ids: Vec<IdType>,
) -> Result<Vec<SpotifyResource>> {
    let mut resources = vec![];
    for id in spotify_ids {
        let resource = match id {
            IdType::Album(i) => {
                let album = client.album(AlbumId::from_id(i)?, None).await?;

                SpotifyResource::Album(album)
            }
            IdType::Track(i) => {
                let track = client.track(TrackId::from_id(i)?, None).await?;

                SpotifyResource::Track(track)
            }
        };

        resources.push(resource);
    }

    Ok(resources)
}

pub(crate) async fn extract_resources(
    spotify_client: &ClientCredsSpotify,
    tidal_client: &TidalClient,
    content: &str,
) -> Vec<ServiceResources> {
    if !contains_spotify_link(content) {
        return vec![];
    }

    let spotify_ids = extract_ids(content);

    let spotify_resources = match get_spotify_resources(spotify_client, spotify_ids.clone()).await {
        Ok(s) => s,
        Err(e) => {
            error!("failed to get spotify_resources: {}", e);
            return vec![];
        }
    };

    match tidal::get_tidal_ids_from_spotify_resources(
        tidal_client,
        spotify_client,
        &spotify_resources,
    )
    .await
    {
        Ok(t) => [
            ServiceResources::Spotify(spotify_ids),
            ServiceResources::Tidal(t),
        ]
        .to_vec(),
        Err(e) => {
            error!("failed to get tidal ids: {}", e);
            [ServiceResources::Spotify(spotify_ids)].to_vec()
        }
    }
}
