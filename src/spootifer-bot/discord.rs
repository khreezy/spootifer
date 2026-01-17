use crate::db::{
    create_auth_request, first_or_create_user_by_discord_user_id,
    first_or_create_user_guild_by_user_id_and_guild_id, get_oauth_token_by_user_id,
    get_user_by_discord_user_id, get_user_by_user_id, get_user_guilds_by_guild_id,
    insert_oauth_token, update_user_guild_spotify_playlist_id, IntoOAuthToken, OAuthToken,
};
use crate::spotify::{
    contains_spotify_link, extract_ids, get_album_images, get_track_ids, init_spotify,
    init_spotify_from_token,
};
use crate::tidal::init_tidal;
use crate::{spotify, tidal};
use async_std::task;
use chrono::{DateTime, Utc};
use log::{error, info};
use oauth2::PkceCodeChallenge;
use rspotify::model::PlaylistId;
use rspotify::prelude::*;
use rspotify::{scopes, ClientCredsSpotify, Token};
use rusqlite::Connection;
use serenity::all::Message;
use serenity::all::ReactionType::Unicode;
use serenity::async_trait;
use serenity::futures::lock;
use serenity::prelude::*;
use std::env;
use std::error::Error;
use std::fmt::{Debug, Display, Formatter};
use std::sync::{Arc, Mutex};
use uuid::Uuid;

pub struct Handler {
    pub(crate) conn: Arc<Mutex<Connection>>,
    pub(crate) spotify_client: Arc<ClientCredsSpotify>,
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
            return;
        }

        info!("got message containing spotify link");

        let guild_id = match new_message.guild_id {
            Some(id) => id,
            None => {
                error!("message not in a guild");
                return;
            }
        };

        let user_guilds =
            match get_user_guilds_by_guild_id(&self.conn, guild_id.to_string().as_str()) {
                Ok(u) => u,
                Err(e) => {
                    error!("error fetching guilds: {}", e);
                    return;
                }
            };

        let spotify_ids = extract_ids(&new_message.content.to_string());

        self.spotify_client
            .request_token()
            .await
            .expect("unable to fetch spotify token");

        let track_ids = get_track_ids(&self.spotify_client, &spotify_ids).await;

        for guild in user_guilds {
            let user = match get_user_by_user_id(&self.conn, guild.user_id) {
                Ok(u) => u,
                Err(e) => {
                    error!("Failed to get user: {:?}", e);
                    continue;
                }
            };

            let user_id = match user.id {
                Some(i) => i,
                None => {
                    error!("user has not been created");
                    continue;
                }
            };

            let spotify_token = match get_oauth_token_by_user_id(&self.conn, user_id) {
                Ok(t) => t,
                Err(e) => {
                    error!("Failed to get spotify token: {:?}", e);
                    continue;
                }
            };

            info!("token expires at: {}", spotify_token.expiry_time);
            let expires_at = match DateTime::parse_from_rfc3339(spotify_token.expiry_time.as_str())
            {
                Ok(t) => t.to_utc(),
                Err(e) => {
                    error!("Error parsing expires at time: {}", e);
                    continue;
                }
            }
            .to_utc();

            let token = Token {
                access_token: spotify_token.access_token,
                refresh_token: Some(spotify_token.refresh_token),
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
                Some(i) => i,
                None => {
                    error!("playlist id not present");
                    continue;
                }
            };

            let playlist_id = match PlaylistId::from_id(&p) {
                Ok(id) => id,
                Err(e) => {
                    error!("Failed to get playlist id: {:?}", e);
                    continue;
                }
            };

            match spotify_client
                .playlist_add_items(playlist_id, track_ids.clone(), None)
                .await
            {
                Ok(_) => {
                    info!("Added tracks to playlist");
                }
                Err(e) => {
                    error!("Failed to add tracks to playlist: {:?}", e);
                    continue;
                }
            };
        }

        let mills500 = std::time::Duration::from_millis(500);
        task::sleep(mills500).await;
        info!("acknowledging message");
        _ = new_message.react(&ctx, Unicode(String::from("✅"))).await;

        let album_image_urls = get_album_images(&self.spotify_client, &spotify_ids).await;

        for image in album_image_urls {
            // Send the image URL as a reply to the original message
            let _ = new_message.reply(&ctx.http, image).await;

            info!("sent track art to channel");
        }
    }
}

#[poise::command(slash_command)]
pub(crate) async fn authorize_spotify<'a>(ctx: CommandCtx<'_>) -> Result<()> {
    let discord_user_str = ctx.author().id.to_string();
    let discord_user_id = discord_user_str.as_str();

    let guild_id = match ctx.guild_id() {
        Some(id) => id.to_string(),
        None => return Err(DiscordError.into()),
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
        None => return Err(DiscordError.into()),
    };

    let _ = match first_or_create_user_guild_by_user_id_and_guild_id(
        &ctx.data().conn,
        guild_id,
        user_id,
    ) {
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

    _ = match create_auth_request(
        &ctx.data().conn,
        spotify_client.oauth.state,
        discord_user_id,
        None,
        None,
        "spotify",
    ) {
        Ok(u) => u,
        Err(e) => {
            error!("error creating auth request: {}", e);
            return Err(DiscordError.into());
        }
    };

    match ctx
        .send(
            poise::CreateReply::default()
                .content(format!(
                    "Please click this link to authorize with spotify.\n{}",
                    auth_url
                ))
                .ephemeral(true),
        )
        .await
    {
        Ok(_) => Ok(()),
        Err(e) => Err(e.into()),
    }
}

#[poise::command(slash_command)]
pub(crate) async fn authorize_tidal<'a>(ctx: CommandCtx<'_>) -> Result<()> {
    let discord_user_str = ctx.author().id.to_string();
    let discord_user_id = discord_user_str.as_str();

    let guild_id = match ctx.guild_id() {
        Some(id) => id.to_string(),
        None => return Err(DiscordError.into()),
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
        None => return Err(DiscordError.into()),
    };

    let _ = match first_or_create_user_guild_by_user_id_and_guild_id(
        &ctx.data().conn,
        guild_id,
        user_id,
    ) {
        Ok(u) => u,
        Err(e) => {
            error!("got error creating user guild: {}", e);
            return Err(DiscordError.into());
        }
    };

    let tidal_client = match init_tidal() {
        Ok(u) => u,
        Err(e) => {
            error!("got error initializing tidal client: {}", e);
            return Err(DiscordError.into());
        }
    };

    let (pkce_code, pkce_verifier) = PkceCodeChallenge::new_random_sha256();

    let state = Uuid::new_v4().to_string();

    let auth_url = tidal_client.get_authorize_url(
        tidal::get_redirect_uri()?.as_str(),
        tidal::DEFAULT_SCOPES,
        pkce_code.as_str(),
        Some(state.as_str()),
    )?;

    _ = match create_auth_request(
        &ctx.data().conn,
        state,
        discord_user_id,
        Some(String::from(pkce_code.as_str())),
        Some(String::from(pkce_verifier.into_secret().as_str())),
        "tidal",
    ) {
        Ok(u) => u,
        Err(e) => {
            error!("error creating auth request: {}", e);
            return Err(DiscordError.into());
        }
    };

    match ctx
        .send(
            poise::CreateReply::default()
                .content(format!(
                    "Please click this link to authorize with tidal.\n{}",
                    auth_url
                ))
                .ephemeral(true),
        )
        .await
    {
        Ok(_) => Ok(()),
        Err(e) => Err(e.into()),
    }
}

#[poise::command(slash_command)]
pub(crate) async fn register_playlist<'a>(
    ctx: CommandCtx<'_>,
    playlist_link: String,
) -> Result<()> {
    let playlist_id = match spotify::extract_playlist_id(playlist_link) {
        Some(id) => id,
        None => {
            error!("failed to parse playlist id");
            let s = ctx
                .say("Check your playlist link, we were not able to parse it.")
                .await;

            return match s {
                Ok(_) => Err(DiscordError.into()),
                Err(e) => Err(e.into()),
            };
        }
    };

    let guild_id = match ctx.guild_id() {
        Some(i) => i,
        None => {
            error!("Failed to get guild id");
            return Err(DiscordError.into());
        }
    }
    .to_string();

    let discord_id = ctx.author().id.to_string();
    let discord_user_id = discord_id.as_str();

    let user = match get_user_by_discord_user_id(&ctx.data().conn, discord_user_id) {
        Ok(u) => u,
        Err(e) => {
            error!("failed to update playlist id: {:?}", e);
            return Err(DiscordError.into());
        }
    };

    let user_id = match user.id {
        Some(i) => i,
        None => return Err(DiscordError.into()),
    };

    _ = match update_user_guild_spotify_playlist_id(
        &ctx.data().conn,
        guild_id,
        user_id,
        playlist_id,
    ) {
        Ok(_) => {}
        Err(e) => {
            error!("Failed to update playlist id: {:?}", e);
            return Err(DiscordError.into());
        }
    };

    match ctx
        .send(
            poise::CreateReply::default()
                .content("Your playlist was registered for this server.")
                .ephemeral(true),
        )
        .await
    {
        Ok(_) => Ok(()),
        Err(e) => Err(e.into()),
    }
}

fn insert_oauth_token_for_discord_user(
    conn: &Arc<Mutex<Connection>>,
    discord_user_id: String,
    token: Box<dyn IntoOAuthToken>,
) -> Result<()> {
    let user_id = match get_user_by_discord_user_id(&conn, discord_user_id.as_str()) {
        Ok(u) => u.id.ok_or("failed to get user id")?,
        Err(e) => return Err(DiscordError.into()),
    };

    let oauth_token = token
        .into_oauth_token(user_id)
        .ok_or("failed to get oauth token from service token")?;

    let mut locked_conn = match conn.try_lock() {
        Ok(c) => c,
        Err(_) => return Err(DiscordError.into()),
    };

    let tx = locked_conn.transaction()?;

    match insert_oauth_token(&tx, oauth_token) {
        Ok(_) => {}
        Err(_) => {
            return Err(DiscordError.into());
        }
    };

    return match tx.commit() {
        Ok(_) => Ok(()),
        Err(_) => Err(DiscordError.into()),
    };
}
