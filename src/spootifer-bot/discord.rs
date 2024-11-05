use std::error::Error;
use std::fmt::{Debug, Display, Formatter};
use chrono::{DateTime};
use rspotify::clients::BaseClient;
use rspotify::model::{AlbumId, PlaylistId, TrackId};
use serenity::all::Message;
use serenity::async_trait;
use serenity::prelude::*;
use rspotify::prelude::*;
use crate::db::{create_auth_request, first_or_create_user_by_discord_user_id, first_or_create_user_guild_by_user_id_and_guild_id, get_spotify_auth_token_by_user_id, get_user_by_discord_user_id, get_user_by_user_id, get_user_guilds_by_guild_id, update_user_guild_spotify_playlist_id};
use crate::spotify::{contains_spotify_link, extract_ids, init_spotify, init_spotify_from_token, is_album};
use log::{info, error};
use rspotify::{scopes, ClientCredsSpotify, Token};
use serenity::all::ReactionType::Unicode;
use std::sync::{Arc, Mutex};
use async_std::task;
use rusqlite::Connection;
use uuid::{Uuid};
use crate::spotify;

pub struct Handler {
    pub(crate) conn: Arc<Mutex<Connection>>,
    pub(crate) spotify_client: Arc<ClientCredsSpotify>
}

struct DiscordError;

impl Debug for DiscordError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "error during discord operations")
    }
}

impl Display for DiscordError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "error during discord operations")
    }
}

type CommandError = Box<dyn Error + Send + Sync>;

type CommandCtx<'a> = poise::Context<'a, Arc<Handler>, CommandError>;
impl Error for DiscordError {}


type Result<T> = std::result::Result<T, Box<dyn Error + Send + Sync>>;

#[async_trait]
impl EventHandler for Handler {
    async fn message(&self, ctx: Context, new_message: Message) {
        if !contains_spotify_link(new_message.content.as_str()) {
            return
        }

        info!("got message containing spotify link");

        let user_guilds = match get_user_guilds_by_guild_id(&self.conn, new_message.guild_id.unwrap().to_string().as_str()) {
            Ok(u) => u,
            Err(e) => {
                error!("error fetching guilds: {}", e);
                return;
            }
        };

        let spotify_ids = extract_ids(&new_message.content.to_string());

        self.spotify_client.request_token().await.expect("unable to fetch spotify token");

        let track_ids: Vec<Option<String>>;

        if is_album(new_message.content.as_str()) {
            info!("fetching album tracks from album: {:?}", spotify_ids);
            track_ids = get_album_track_ids(&self.spotify_client, spotify_ids.get(0)).await
        } else {
            track_ids = spotify_ids.into_iter().map(|id| -> Option<String> {
                Some(id)
            }).collect()
        }

        info!("got track ids: {:?}", track_ids);

        let filtered_track_ids: Vec<PlayableId> = track_ids.into_iter().filter_map(|x| match x {
            Some(s) => match TrackId::from_id(s) { Ok(t) => Some(PlayableId::from(t)), Err(_) => None },
            None => None
        }).collect();

        for guild in user_guilds {
            let user = match get_user_by_user_id(&self.conn, guild.user_id) {
                Ok(u) => u,
                Err(e) => {
                    error!("Failed to get user: {:?}", e);
                    continue
                }
            };

            let user_id = match user.id {
                Some(i) => i,
                None => {
                    error!("user has not been created");
                    continue
                }
            };

            let spotify_token = match get_spotify_auth_token_by_user_id(&self.conn, user_id) {
                Ok(t) => t,
                Err(e) => {
                    error!("Failed to get spotify token: {:?}", e);
                    continue
                }
            };

            info!("token expires at: {}", spotify_token.spotify_expiry_time);
            let expires_at = match DateTime::parse_from_rfc3339(spotify_token.spotify_expiry_time.as_str()) {
                Ok(t) => t.to_utc(),
                Err(e) => {
                    error!("Error parsing expires at time: {}", e);
                    continue
                }
            }.to_utc();

            let token = Token {
                access_token: spotify_token.spotify_access_token,
                refresh_token: Some(spotify_token.spotify_refresh_token),
                expires_at: Some(expires_at),
                expires_in: Default::default(),
                scopes: scopes!("playlist-modify-public"),
            };

            let spotify_client = match init_spotify_from_token(token) {
                Ok(c) => c,
                Err(e) => {
                    error!("error getting spotify client: {}", e);
                    return;
                }
            };

            let p = match guild.spotify_playlist_id {
                Some(i) => { i }
                None => { error!("playlist id not present"); continue }
            };

            let playlist_id = match PlaylistId::from_id(&p) {
                Ok(id) => id,
                Err(e) => {
                    error!("Failed to get playlist id: {:?}", e);
                    continue
                }
            };

            match spotify_client.playlist_add_items(playlist_id, filtered_track_ids.clone(), None).await {
                Ok(_) => {
                    info!("Added tracks to playlist");
                },
                Err(e) => {
                    error!("Failed to add tracks to playlist: {:?}", e);
                    continue
                }
            };
        }

        let mills500 = std::time::Duration::from_millis(500);
        task::sleep(mills500).await;
        info!("acknowledging message");
        _ = new_message.react(ctx, Unicode(String::from("✅"))).await;
    }
}

#[poise::command(slash_command)]
pub(crate) async fn authorize_spotify<'a>(ctx: CommandCtx<'_>) -> Result<()>  {
    let discord_user_str = ctx.author().id.to_string();
    let discord_user_id = discord_user_str.as_str();

    let guild_id = match ctx.guild_id() {
        Some(id) => id.to_string(),
        None => return Err(DiscordError.into())
    };

    let user = match first_or_create_user_by_discord_user_id(&ctx.data().conn, discord_user_id) {
        Ok(u) => u,
        Err(e) => {
            error!("error creating user: {}", e);
            return Err(DiscordError.into());
        }
    };

    let user_id = match user.id {
        Some(i) => i,
        None => { return Err(DiscordError.into()) }
    };

    let _ = match first_or_create_user_guild_by_user_id_and_guild_id(&ctx.data().conn, guild_id, user_id) {
        Ok(u) => u,
        Err(e) => {
            error!("got error creating user guild: {}", e);
            return Err(DiscordError.into());
        }
    };

    let mut spotify_client = match init_spotify() {
        Ok(u) => u,
        Err(e) => {
            error!("got error initializing spotify client: {}", e);
            return Err(DiscordError.into());
        }
    };

    spotify_client.oauth.state = Uuid::new_v4().to_string();

    let auth_url = match spotify_client.get_authorize_url(false) {
        Ok(u) => u,
        Err(e) => {
            error!("error getting auth url: {}", e);
            return Err(DiscordError.into());
        }
    };

    _ = match create_auth_request(&ctx.data().conn, spotify_client.oauth.state, discord_user_id) {
        Ok(u) => u,
        Err(e) => {
            error!("error creating auth request: {}", e);
            return Err(DiscordError.into());
        }
    };

    match ctx.send(poise::CreateReply::default().content(format!("Please click this link to authorize with spotify.\n{}", auth_url)).ephemeral(true)).await {
        Ok(_) => Ok(()),
        Err(e) => Err(e.into())
    }
}

#[poise::command(slash_command)]
pub(crate) async fn register_playlist<'a>(ctx: CommandCtx<'_>, playlist_link: String) -> Result<()>{
    let playlist_id = match spotify::extract_playlist_id(playlist_link) {
        Some(id) => id,
        None => {
            error!("failed to parse playlist id");
            let s = ctx.say("Check your playlist link, we were not able to parse it.").await;

            return match s {
                Ok(_) => Err(DiscordError.into()),
                Err(e) => Err(e.into())
            };
        }
    };

    let guild_id = match ctx.guild_id() {
        Some(i) => i,
        None => {
            error!("Failed to get guild id");
            return Err(DiscordError.into())
        }
    }.to_string();

    let discord_id = ctx.author().id.to_string();
    let discord_user_id = discord_id.as_str();

    let user = match get_user_by_discord_user_id(&ctx.data().conn, discord_user_id) {
        Ok(u) => u,
        Err(e) => { error!("failed to update playlist id: {:?}", e); return Err(DiscordError.into())}
    };

    let user_id = match user.id {
        Some(i) => i,
        None => { return Err(DiscordError.into()) }
    };

    _ = match update_user_guild_spotify_playlist_id(&ctx.data().conn, guild_id, user_id, playlist_id) {
        Ok(_) => {},
        Err(e) => { error!("Failed to update playlist id: {:?}", e); return Err(DiscordError.into()) }
    };

    match ctx.send(poise::CreateReply::default().content("Your playlist was registered for this server.").ephemeral(true)).await {
        Ok(_) => Ok(()),
        Err(e) => Err(e.into())
    }
}

async fn get_album_track_ids(client: &Arc<ClientCredsSpotify>, album_id: Option<&String>) -> Vec<Option<String>> {
    let unwrapped_id = match album_id {
        None => { info!("not an album id?"); return vec![None] },
        Some(i) => i
    };

    let album_id = match AlbumId::from_id(unwrapped_id) {
        Ok(id) => id,
        Err(e) => {
            error!("Failed to get album id: {}", e.to_string());
            return vec![None]
        }
    };

    let album = client.album(album_id, None).await.unwrap();
    album.tracks.items.into_iter().map(|t| -> Option<String> {
        match t.id {
            Some(id) => Some(id.to_string().replace("spotify:track:", "")),
            None => { error!("couldn't get track id"); None }
        }
    }).collect()
}

