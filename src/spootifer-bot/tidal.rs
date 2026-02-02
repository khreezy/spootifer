use crate::discord::ServiceResources;
use crate::error;
use crate::spotify::{IdType, SpotifyResource};
use log::{info, warn};
use prawn::apis::Api;
use prawn::client::{
    OAuthConfig, RetryConfig, TidalClient, TidalClientConfig, TidalClientError, Token,
};
use prawn::models::{
    AlbumsAttributes, AlbumsResourceObject, AlbumsSingleResourceDataDocument, IncludedInner,
    TracksAttributes, TracksSingleResourceDataDocument,
};
use regex::Regex;
use rspotify::ClientCredsSpotify;
use rspotify::model::{FullAlbum, FullTrack, Id, SimplifiedAlbum};
use rspotify::prelude::BaseClient;
use std::env;
use std::error::Error;
use std::fmt::{Display, Formatter};
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
        retry_config: Some(RetryConfig {}),
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
        retry_config: Some(RetryConfig {}),
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
        retry_config: Some(RetryConfig {}),
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

pub(crate) fn extract_ids(link: &str) -> Vec<TidalResource> {
    let re = match Regex::new(
        r"(((?:https://tidal\.com/track/|https://tidal\.com/album/)([a-zA-Z0-9]+)))/u",
    ) {
        Ok(re) => re,
        Err(e) => {
            error!("Failed to compile regex: {}", e);
            return vec![];
        }
    };

    let matches = re.captures_iter(link);

    return matches
        .filter_map(|m| -> Option<Vec<TidalResource>> {
            if m.len() > 1 {
                let link = m.get(1)?.as_str();

                if is_album(link) {
                    Some(vec![TidalResource::Album(m.get(3)?.as_str().to_string())])
                } else {
                    Some(vec![TidalResource::Track(m.get(3)?.as_str().to_string())])
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
    info!("getting tracks for album {}", album_id);
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

    info!(
        "got back album with at least {} tracks",
        album_tracks_data.len(),
    );
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

        maybe_next = album_tracks.links.meta
    }

    Ok(track_ids)
}
pub(crate) async fn get_track_ids(
    client: &TidalClient,
    tidal_ids: &Vec<TidalResource>,
) -> Result<Vec<String>> {
    info!("resolving track ids for {} resources", tidal_ids.len());
    let mut track_ids = vec![];

    for resource in tidal_ids {
        match resource {
            TidalResource::Album(id) => match &mut get_album_track_ids(client, id.clone()).await {
                Ok(v) => track_ids.append(v),
                Err(e) => {
                    error!("failed to fetch album ids: {}", e);
                    continue;
                }
            },
            TidalResource::Track(id) => track_ids.push(id.clone()),
        }
    }

    Ok(track_ids)
}

#[derive(Clone)]
pub enum TidalResource {
    Album(String),
    Track(String),
}

pub(crate) async fn get_tidal_ids_from_spotify_resources(
    tidal_client: &TidalClient,
    spotify_client: &ClientCredsSpotify,
    spotify_resources: &Vec<SpotifyResource>,
) -> Result<Vec<TidalResource>> {
    let mut ids = vec![];

    for resource in spotify_resources {
        let Some(resource) = (match resource {
            SpotifyResource::Album(album) => match_album(tidal_client, album)
                .await
                .map_or(None, |id: String| -> Option<TidalResource> {
                    Some(TidalResource::Album(id.clone()))
                }),
            SpotifyResource::Track(track) => match_track(tidal_client, spotify_client, &track)
                .await
                .map_or(None, |id: String| -> Option<TidalResource> {
                    Some(TidalResource::Track(id.clone()))
                }),
        }) else {
            warn!("failed to match a tidal resource");
            continue;
        };

        ids.push(resource)
    }

    Ok(ids)
}

async fn match_album(tidal_client: &TidalClient, album: &FullAlbum) -> Option<String> {
    let artist = match album.artists.first() {
        Some(a) => a.name.as_str(),
        None => "",
    };

    let search_string = album.name.clone() + " " + artist;

    let maybe_matched_album = match_album_with_search(tidal_client, album, search_string).await;

    if maybe_matched_album.is_some() {
        return maybe_matched_album;
    }

    let search_string_no_artist = album.name.clone();

    match_album_with_search(tidal_client, album, search_string_no_artist).await
}

async fn match_album_with_search(
    tidal_client: &TidalClient,
    album: &FullAlbum,
    search_string: String,
) -> Option<String> {
    let search = match tidal_client
        .search_results_api()
        .get_search_result_albums(
            search_string.as_str(),
            Some("INCLUDE"),
            None,
            None,
            Some(vec![String::from("albums")]),
        )
        .await
    {
        Ok(s) => s,
        Err(e) => {
            error!("failed to do album search: {}", e);
            return None;
        }
    };

    let Some(top_albums) = search.included else {
        error!("relationships missing");
        return None;
    };

    info!("{} album results", top_albums.len());

    let Some(IncludedInner::Albums(top_album)) = top_albums.iter().find(|i| -> bool {
        let IncludedInner::Albums(tidal_album) = i else {
            return false;
        };

        let Some(attrs) = tidal_album.attributes.clone() else {
            return false;
        };
        album_matches(attrs.as_ref(), album)
    }) else {
        error!("no album matched search {}", search_string);
        return None;
    };

    Some(top_album.id.clone())
}

fn album_matches(attrs: &AlbumsAttributes, full_spotify_album: &FullAlbum) -> bool {
    let upc_matches = full_spotify_album
        .external_ids
        .get("upc")
        .is_some_and(|v| *v == attrs.barcode_id);
    let ean_matches = full_spotify_album
        .external_ids
        .get("ean")
        .is_some_and(|v| *v == attrs.barcode_id);

    let barcode_matches = upc_matches || ean_matches;

    info!(
        "tidal name {} spotify name {}",
        attrs.title, full_spotify_album.name,
    );
    barcode_matches || attrs.title == full_spotify_album.name
}

fn normalize_track_name(name: String) -> String {
    name.replace("-", "")
        .replace("(", "")
        .replace(")", "")
        .replace("  ", " ")
}

fn track_matches(tidal_track: &TracksAttributes, spotify_track: &FullTrack) -> bool {
    info!(
        "tidal track name {} spotify track name {}",
        tidal_track.title, spotify_track.name
    );
    let maybe_spotify_isrc = spotify_track.external_ids.get(&"isrc".to_string());
    let maybe_tidal_duration = iso8601::duration(tidal_track.duration.as_str());
    let normalize_tidal_track_name = normalize_track_name(tidal_track.title.clone());
    let normalized_spotify_track_name = normalize_track_name(spotify_track.name.clone());
    (maybe_spotify_isrc.is_some() && *maybe_spotify_isrc.unwrap() == tidal_track.isrc)
        || (maybe_tidal_duration.is_ok_and(|d| -> bool {
            spotify_track
                .duration
                .num_seconds()
                .checked_sub_unsigned(Duration::from(d).as_secs())
                .unwrap_or(1000)
                .abs()
                < 2
        }) && normalize_tidal_track_name == normalized_spotify_track_name)
}

fn track_matches_in_list(maybe_track: &&IncludedInner, spotify_track: &FullTrack) -> bool {
    let IncludedInner::Tracks(track) = maybe_track else {
        return false;
    };

    let Some(attrs) = track.attributes.clone() else {
        return false;
    };

    track_matches(attrs.as_ref(), spotify_track)
}

async fn find_track_in_album(
    client: &TidalClient,
    spotify_client: &ClientCredsSpotify,
    track: &FullTrack,
) -> Option<String> {
    let album_name = track.album.name.clone();
    let artist_name = track.artists.first()?.name.clone();

    let search_string = album_name + " " + artist_name.as_str();

    let search = match client
        .search_results_api()
        .get_search_result_albums(
            search_string.as_str(),
            Some("INCLUDE"),
            None,
            None,
            Some(vec![String::from("albums")]),
        )
        .await
    {
        Ok(s) => s,
        Err(e) => {
            error!("failed to do search: {}", e);
            return None;
        }
    };

    let album_id = track.album.id.clone();

    let full_album = if let Some(id) = album_id {
        let album_resp = spotify_client.album(id, None).await;
        match album_resp {
            Ok(a) => a,
            Err(e) => {
                error!("failed to get spotify album: {}", e);
                return None;
            }
        }
    } else {
        error!("album id not present");
        return None;
    };

    let albums = search.included?;
    let Some(IncludedInner::Albums(top_album)) = albums.iter().find(|a| -> bool {
        let IncludedInner::Albums(album) = a else {
            return false;
        };

        let Some(attrs) = album.attributes.clone() else {
            return false;
        };
        album_matches(attrs.as_ref(), &full_album)
    }) else {
        warn!("failed to match album");
        return None;
    };

    let top_album_name = top_album.attributes.as_ref().unwrap().title.clone();

    info!("matched album id {} name {}", top_album.id, top_album_name);

    let Ok(album_tracks_resp) = client
        .albums_api()
        .get_album(&top_album.id, None, Some(vec!["items".to_string()]), None)
        .await
    else {
        error!("failed to get album items");
        return None;
    };

    let album_tracks = album_tracks_resp.included?;

    info!(
        "trying to match album tracks for returned album {}",
        album_tracks_resp.data.attributes.unwrap().title
    );

    let Some(IncludedInner::Tracks(matched_track)) = album_tracks
        .iter()
        .find(|t| -> bool { track_matches_in_list(t, track) })
    else {
        warn!("failed to match a  track");
        return None;
    };

    Some(matched_track.id.clone())
}

async fn find_track(client: &TidalClient, track: &FullTrack) -> Option<String> {
    let track_name = track.name.clone();
    let artist_name = track.artists.first()?.name.clone();

    let search_string = track_name + " " + artist_name.as_str();

    let search = match client
        .search_results_api()
        .get_search_result_tracks(
            search_string.as_str(),
            Some("INCLUDE"),
            None,
            None,
            Some(vec![String::from("tracks")]),
        )
        .await
    {
        Ok(s) => s,
        Err(e) => {
            error!("failed to do search: {}", e);
            return None;
        }
    };

    let search_included = search.included?;
    let track_ref = &track;

    let Some(IncludedInner::Tracks(found_track)) = search_included
        .iter()
        .find(|t| -> bool { track_matches_in_list(t, track_ref) })
    else {
        warn!("failed to match a track");
        return None;
    };

    Some(found_track.id.clone())
}

async fn match_track(
    client: &TidalClient,
    spotify_client: &ClientCredsSpotify,
    track: &FullTrack,
) -> Option<String> {
    let track_in_album = find_track_in_album(client, spotify_client, track).await;

    if track_in_album.is_some() {
        return track_in_album;
    }

    find_track(client, track).await
}

pub enum FullTidalResource {
    Track(TracksSingleResourceDataDocument),
    Album(AlbumsSingleResourceDataDocument),
}

pub async fn get_full_tidal_resources(
    client: &TidalClient,
    resources: Vec<TidalResource>,
) -> Vec<FullTidalResource> {
    let mut full_resources = vec![];
    for resource in resources {
        match resource {
            TidalResource::Album(album_id) => {
                match client
                    .albums_api()
                    .get_album(
                        album_id.as_str(),
                        None,
                        Some(vec!["artists".to_string()]),
                        None,
                    )
                    .await
                {
                    Ok(a) => full_resources.push(FullTidalResource::Album(a)),
                    Err(e) => {
                        error!("error fetching tidal album from api: {}", e);
                        continue;
                    }
                }
            }
            TidalResource::Track(track_id) => {
                match client
                    .tracks_api()
                    .get_track(
                        track_id.as_str(),
                        None,
                        Some(vec!["artists".to_string()]),
                        None,
                    )
                    .await
                {
                    Ok(t) => full_resources.push(FullTidalResource::Track(t)),
                    Err(e) => {
                        error!("error fetching tidal track from api: {}", e);
                        continue;
                    }
                }
            }
        }
    }
    full_resources
}

async fn match_spotify_album(
    spotify_client: &ClientCredsSpotify,
    tidal_album: AlbumsSingleResourceDataDocument,
) -> Option<IdType> {
    let album_attrs = tidal_album.data.attributes?;

    let included = tidal_album.included?;
    let IncludedInner::Artists(artist) = included.first()? else {
        return None;
    };

    let query_string = format!(
        "album={}&upc={}&artist={}",
        album_attrs.title,
        album_attrs.barcode_id,
        artist.attributes.clone()?.name
    );

    let rspotify::model::SearchResult::Albums(albums_search) = (match spotify_client
        .search(
            query_string.as_str(),
            rspotify::model::SearchType::Album,
            None,
            None,
            None,
            None,
        )
        .await
    {
        Ok(s) => s,
        Err(e) => {
            error!("failed to search for spotify album: {}", e);
            return None;
        }
    }) else {
        return None;
    };

    let mut full_albums = vec![];

    for simplified_album in albums_search.items {
        let Some(id) = simplified_album.id else {
            continue;
        };
        let Ok(full_album) = spotify_client.album(id, None).await else {
            continue;
        };

        full_albums.push(full_album)
    }

    let Some(top_result) = full_albums
        .iter()
        .find(|t| -> bool { album_matches(album_attrs.as_ref(), t) })
    else {
        warn!("no album found");
        return None;
    };

    let id = top_result.id.to_string().replace("spotify:album:", "");
    info!("matched album id: {}", id);

    Some(IdType::Album(id))
}

async fn match_spotify_track(
    spotify_client: &ClientCredsSpotify,
    tidal_track: TracksSingleResourceDataDocument,
) -> Option<IdType> {
    let Some(track_attrs) = tidal_track.data.attributes else {
        info!("no attrs on track");
        return None;
    };

    let Some(included) = tidal_track.included else {
        info!("no included data");
        return None;
    };
    let IncludedInner::Artists(artist) = included.first()? else {
        info!("no artist info");
        return None;
    };

    let query_string = format!(
        "album={}&isrc={}&artist={}",
        track_attrs.title,
        track_attrs.isrc,
        artist.attributes.clone()?.name
    );

    let rspotify::model::SearchResult::Tracks(tracks_search) = (match spotify_client
        .search(
            query_string.as_str(),
            rspotify::model::SearchType::Track,
            None,
            None,
            None,
            None,
        )
        .await
    {
        Ok(s) => s,
        Err(e) => {
            error!("failed to search for spotify album: {}", e);
            return None;
        }
    }) else {
        info!("was not a track search  result");
        return None;
    };

    let Some(top_result) = tracks_search
        .items
        .iter()
        .find(|t| -> bool { track_matches(track_attrs.as_ref(), t) })
    else {
        warn!("no album found");
        return None;
    };

    let Some(id) = top_result.id.clone() else {
        info!("no id on result");
        return None;
    };

    let id = id.to_string().replace("spotify:track:", "");
    info!("matched track id: {}", id);

    Some(IdType::Track(id))
}

async fn match_spotify_resources(
    tidal_client: &TidalClient,
    spotify_client: &ClientCredsSpotify,
    tidal_resources: Vec<FullTidalResource>,
) -> Vec<IdType> {
    let mut spotify_resource = vec![];
    for resource in tidal_resources {
        match resource {
            FullTidalResource::Album(album) => {
                let Some(matched_album) = match_spotify_album(spotify_client, album).await else {
                    warn!("no album matched");
                    continue;
                };

                spotify_resource.push(matched_album);
            }
            FullTidalResource::Track(track) => {
                let Some(matched_track) = match_spotify_track(spotify_client, track).await else {
                    warn!("no track matched");
                    continue;
                };

                spotify_resource.push(matched_track)
            }
        }
    }

    spotify_resource
}

pub async fn extract_resources(
    tidal_client: &TidalClient,
    spotify_client: &ClientCredsSpotify,
    msg: &str,
) -> Vec<ServiceResources> {
    if !contains_tidal_link(msg.to_string()) {
        return vec![];
    }

    let tidal_resources = extract_ids(msg);

    let full_tidal_resources =
        get_full_tidal_resources(tidal_client, tidal_resources.clone()).await;

    let spotify_resources =
        match_spotify_resources(tidal_client, spotify_client, full_tidal_resources).await;

    if spotify_resources.len() > 0 {
        return [
            ServiceResources::Tidal(tidal_resources),
            ServiceResources::Spotify(spotify_resources),
        ]
        .to_vec();
    }

    [ServiceResources::Tidal(tidal_resources)].to_vec()
}
