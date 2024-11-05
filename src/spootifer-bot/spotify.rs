use std::env;
use std::error::Error;
use std::fmt::{Display, Formatter};
use log::error;
use rspotify::{scopes, AuthCodeSpotify, Config, Credentials, OAuth, Token};
use regex::Regex;

const SPOTIFY_DOMAIN: &str  = "open.spotify.com";
const SPOTIFY_SHORTENED_DOMAIN: &str = "spotify.link";
const SPOTIFY_ALBUM_URI: &str = "spotify:album:";
const MAX_REDIRECT_DEPTH: u32 = 5;

const SPOTIFY_ALBUM_LINK: &str = "https://open.spotify.com/album/";

type Result<T> = std::result::Result<T, Box<dyn Error>>;

pub(crate) fn is_album(link: &str) -> bool {
    link.contains(SPOTIFY_ALBUM_LINK)
}

pub(crate) fn contains_spotify_link(msg: &str) -> bool {
    msg.contains(SPOTIFY_DOMAIN)
}



pub(crate) fn extract_ids<'a>(link: &'a String) -> Vec<String> {
    let re = Regex::new(r"(((?:https?://open\.spotify\.com/track/|https?://open\.spotify\.com/album/|spotify:track:|spotify:album:)([a-zA-Z0-9]+))|https?://spotify.link/[a-zA-Z0-9]+)").unwrap();

    let matches = re.captures_iter(link.as_str());

    return matches.filter_map(|m| -> Option<Vec<String>> {
        if m.len() > 1 {
            let link = m.get(1).unwrap().as_str().to_string();

            if link.contains(SPOTIFY_SHORTENED_DOMAIN) {
                let full_url: String = match expand_spotify_short_link(link, 0) {
                    Ok(url) => url,
                    Err(_) => {
                        return None
                    }
                };

                let ids = extract_ids(&full_url);

                return Some(ids)
            } else {
                return Some(vec![m.get(3).unwrap().as_str().to_string()])
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

fn expand_spotify_short_link<'a>(link: String, depth: u32) -> Result<String> {
    if depth >= MAX_REDIRECT_DEPTH {
        let link_string = link.to_string();
        return Ok(link_string);
    }

    let result = reqwest::blocking::Client::new().get(link)
        .header("User-Agent", "python-requests/2.31.0")
        .header("Accept-Encoding", "gzip, deflate")
        .header("Accept", "*/*")
        .header("Connection", "keep-alive")
        .send()?;

    let expanded_url = result.url();

    if expanded_url.as_str().contains(SPOTIFY_DOMAIN) {
        return expand_spotify_short_link(expanded_url.to_string(), depth);
    }

    let expanded_url_string = expanded_url.to_string();
    
    Ok(expanded_url_string)
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