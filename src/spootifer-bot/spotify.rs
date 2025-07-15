use std::env;
use std::error::Error;
use std::fmt::{Display, Formatter};
use log::error;
use rspotify::{scopes, AuthCodeSpotify, ClientCredsSpotify, Config, Credentials, OAuth, Token};
use rspotify::model::{AlbumId, TrackId, FullAlbum, FullTrack, Image};
use rspotify::clients::BaseClient;
use regex::Regex;

const SPOTIFY_DOMAIN: &str  = "open.spotify.com";
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



pub(crate) fn extract_ids(link: &str) -> Vec<String> {
    let re = Regex::new(r"(((?:https?://open\.spotify\.com/track/|https?://open\.spotify\.com/album/|spotify:track:|spotify:album:)([a-zA-Z0-9]+))|https?://spotify.link/[a-zA-Z0-9]+)").unwrap();

    let matches = re.captures_iter(link);

    return matches.filter_map(|m| -> Option<Vec<String>> {
        if m.len() > 1 {
            let link = m.get(1).unwrap().as_str();

            return if link.contains(SPOTIFY_SHORTENED_DOMAIN) {
                let full_url: String = match expand_spotify_short_link(link, 0) {
                    Ok(url) => url,
                    Err(_) => {
                        return None
                    }
                };

                let ids = extract_ids(&full_url);

                Some(ids)
            } else {
                Some(vec![m.get(3).unwrap().as_str().to_string()])
            }
        } else {
            None
        }
    }).flat_map(|x| { x }).collect();
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

    let result = reqwest::blocking::Client::new().get(link)
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
        scopes: scopes!(
            "playlist-modify-public"
        ),
        redirect_uri: env::var("SPOTIFY_REDIRECT_URI").unwrap_or_else(|_| SPOTIFY_DOMAIN.to_string()),
        ..Default::default()
    };
    
    let creds =  match Credentials::from_env() {
        Some(creds) => creds,
        None => {
            error!("Spotify credentials not set");
            return Err(SpotifyErr.into())
        }
    };
    
    Ok(AuthCodeSpotify::from_token_with_config(token, creds, oauth, config))
}

pub(crate) fn init_spotify() -> Result<AuthCodeSpotify> {
    let config = Config {
        ..Default::default()
    };

    // Please notice that protocol of redirect_uri, make sure it's http (or
    // https). It will fail if you mix them up.
    let oauth = OAuth {
        scopes: scopes!(
            "playlist-modify-public"
        ),
        redirect_uri: env::var("SPOTIFY_REDIRECT_URI").unwrap_or_else(|_| SPOTIFY_DOMAIN.to_string()),
        ..Default::default()
    };

    let creds = match Credentials::from_env() {
        Some(creds) => creds,
        None => {
            error!("Spotify credentials not set");
            return Err(SpotifyErr.into())
        }
    };

    Ok(AuthCodeSpotify::with_config(creds, oauth, config))
}

pub(crate) fn extract_playlist_id(link: String) -> Option<String> {
    let re = Regex::new(r"https://open\.spotify\.com/playlist/([a-zA-Z0-9]+)").expect("unable to compile regex");

    Some(re.captures(link.as_str())?.get(1)?.as_str().to_string())
}

pub(crate) fn extract_track_id(link: &str) -> Option<String> {
    let re = Regex::new(r"https://open\.spotify\.com/track/([a-zA-Z0-9]+)").expect("unable to compile regex");

    Some(re.captures(link)?.get(1)?.as_str().to_string())
}

pub(crate) async fn get_album_cover_image_from_track(spotify: &AuthCodeSpotify, track_id: &str) -> Result<Option<Image>> {
    let track_id = TrackId::from_id(track_id)?;
    
    let track: FullTrack = spotify.track(track_id, None).await?;
    
    Ok(track.album.images.into_iter().next())
}

pub(crate) async fn get_album_cover_image_from_track_creds(spotify: &ClientCredsSpotify, track_id: &str) -> Result<Option<Image>> {
    let track_id = TrackId::from_id(track_id)?;
    
    let track: FullTrack = spotify.track(track_id, None).await?;
    
    Ok(track.album.images.into_iter().next())
}

pub(crate) fn extract_album_id(link: &str) -> Option<String> {
    let re = Regex::new(r"https://open\.spotify\.com/album/([a-zA-Z0-9]+)").expect("unable to compile regex");

    Some(re.captures(link)?.get(1)?.as_str().to_string())
}

pub(crate) async fn get_album_cover_image(spotify: &AuthCodeSpotify, album_id: &str) -> Result<Option<Image>> {
    let album_id = AlbumId::from_id(album_id)?;
    
    let album: FullAlbum = spotify.album(album_id, None).await?;
    
    Ok(album.images.into_iter().next())
}

pub(crate) async fn get_album_cover_image_creds(spotify: &ClientCredsSpotify, album_id: &str) -> Result<Option<Image>> {
    let album_id = AlbumId::from_id(album_id)?;
    
    let album: FullAlbum = spotify.album(album_id, None).await?;
    
    Ok(album.images.into_iter().next())
}