use isopod::client::{OAuthConfig, RetryConfig, Token, YoutubeClient, YoutubeClientConfig};
use log::error;
use regex::Regex;
use std::env;
use std::error::Error;
use url::Url;

use crate::discord::ServiceResources;

pub static DEFAULT_SCOPES: &[&str] = &["https://www.googleapis.com/auth/youtube"];

pub static YOUTUBE_DOMAIN: &str = "youtube.com";
pub static SHORT_YOUTUBE_DOMAIN: &str = "youtu.be";

type Result<T> = std::result::Result<T, Box<dyn Error + Send + Sync>>;

#[derive(Clone, Debug)]
pub enum YoutubeResource {
    Video(String),
}

pub fn get_redirect_uri() -> Result<String> {
    let base_uri = env::var("BASE_REDIRECT_URI")?;

    Ok(format!("{base_uri}/callback"))
}

pub fn init_youtube() -> Result<YoutubeClient> {
    let client_id = env::var("YOUTUBE_CLIENT_ID")?;
    let client_secret = env::var("YOUTUBE_CLIENT_SECRET")?;

    let redirect_uri = get_redirect_uri()?;

    let config = YoutubeClientConfig {
        oauth_config: OAuthConfig {
            redirect_uri,
            client_id,
            client_secret,
        },
        auth_token: None,
        retry_config: Some(RetryConfig {}),
    };

    Ok(isopod::client::YoutubeClient::new(config)?)
}

pub fn init_youtube_with_token(token: Token) -> Result<YoutubeClient> {
    let client_id = env::var("TIDAL_CLIENT_ID")?;
    let client_secret = env::var("YOUTUBE_CLIENT_SECRET")?;

    let redirect_uri = get_redirect_uri()?;

    let config = YoutubeClientConfig {
        oauth_config: OAuthConfig {
            redirect_uri,
            client_id,
            client_secret,
        },
        auth_token: Some(token),
        retry_config: Some(RetryConfig {}),
    };

    Ok(isopod::client::YoutubeClient::new(config)?)
}

pub fn contains_youtube_link(link: &str) -> bool {
    link.contains(YOUTUBE_DOMAIN) || link.contains(SHORT_YOUTUBE_DOMAIN)
}

pub fn extract_playlist_id(link: &str) -> Option<String> {
    let Ok(url) = Url::parse(link) else {
        error!("failed to parse playlist link");
        return None;
    };

    let Some((_, id)) = url
        .query_pairs()
        .find(|(name, _)| -> bool { name == "list" })
    else {
        error!("link did not contain playlist query param");
        return None;
    };

    Some(id.to_string())
}

pub fn extract_ids(link: &str) -> Vec<YoutubeResource> {
    let re = match Regex::new(
        r"(?<link>(?<desktop>https://www\.youtube\.com/watch\?[a-zA-Z0-9%=&_-]+)|(?<withid>(?:https://youtu\.be/|https://www\.youtube\.com/live/)(?<id>[-a-zA-Z0-9]{11})))+",
    ) {
        Ok(re) => re,
        Err(e) => {
            error!("Failed to compile regex: {e}");
            return vec![];
        }
    };

    let matches = re.captures_iter(link);

    matches
        .filter_map(|m| -> Option<YoutubeResource> {
            if let Some(desktop) = m.name("desktop") {
                let Ok(link) = Url::parse(desktop.as_str()) else {
                    error!("{} was not a url", desktop.as_str());
                    return None;
                };

                let Some((_, id)) = link.query_pairs().find(|(key, _)| -> bool { key == "v" })
                else {
                    error!("no video id in link {link}");
                    return None;
                };

                Some(YoutubeResource::Video(id.to_string()))
            } else {
                m.name("id").map(|id| -> YoutubeResource {
                    YoutubeResource::Video(id.as_str().to_string())
                })
            }
        })
        .collect()
}

pub fn extract_resources(link: &str) -> Vec<ServiceResources> {
    if !contains_youtube_link(link) {
        return vec![];
    }
    vec![ServiceResources::Youtube(extract_ids(link))]
}
