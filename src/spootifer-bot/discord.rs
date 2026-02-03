use crate::db::{
    create_auth_request, first_or_create_user_by_discord_user_id,
    first_or_create_user_guild_by_user_id_and_guild_id, get_oauth_token_by_user_id_and_service,
    get_user_by_discord_user_id, get_user_by_user_id, get_user_guilds_by_guild_id_and_service,
    update_user_guild_playlist_id,
};
use crate::spotify::{
    IdType, get_album_images, get_track_ids, init_spotify, init_spotify_from_token,
};
use crate::tidal::{TidalResource, init_tidal};
use crate::youtube::{self, YoutubeResource, init_youtube};
use crate::{spotify, tidal};
use async_std::task;
use chrono::{DateTime, TimeDelta};
use isopod::apis::Api as IsopodApi;
use isopod::models::{PlaylistItem, PlaylistItemSnippet, ResourceId};
use log::{error, info};
use prawn::apis::Api as PrawnApi;
use prawn::client::{TidalClient, Token};
use prawn::models::{
    self, PlaylistItemsRelationshipAddOperationPayload,
    PlaylistItemsRelationshipAddOperationPayloadData,
};
use rspotify::model::PlaylistId;
use rspotify::prelude::*;
use rspotify::{ClientCredsSpotify, scopes};
use rusqlite::Connection;
use serenity::all::Message;
use serenity::all::ReactionType::Unicode;
use serenity::async_trait;
use serenity::prelude::*;
use std::error::Error;
use std::fmt::{Debug, Display, Formatter};
use std::sync::{Arc, Mutex};
use uuid::Uuid;

#[derive(Clone)]
pub struct Handler {
    pub(crate) conn: Arc<Mutex<Connection>>,
    pub(crate) spotify_client: Arc<ClientCredsSpotify>,
    pub(crate) tidal_client: Arc<TidalClient>,
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

pub type CommandCtx<'a> = poise::Context<'a, Arc<Handler>, CommandError>;

impl Error for DiscordError {}

type Result<T> = std::result::Result<T, Box<dyn Error + Send + Sync>>;

#[derive(Clone)]
pub enum ServiceResources {
    Spotify(Vec<IdType>),
    Tidal(Vec<TidalResource>),
    Youtube(Vec<YoutubeResource>),
}

#[async_trait]
impl EventHandler for Handler {
    async fn message(&self, ctx: Context, new_message: Message) {
        let content = new_message.content.clone();

        let resources = [
            spotify::extract_resources(
                self.spotify_client.as_ref(),
                self.tidal_client.as_ref(),
                content.as_str(),
            )
            .await,
            tidal::extract_resources(
                self.tidal_client.as_ref(),
                self.spotify_client.as_ref(),
                content.as_str(),
            )
            .await,
            youtube::extract_resources(content.as_str()),
        ]
        .concat();

        info!("processing {} resource sets", resources.len());

        for resource_set in resources.clone() {
            match resource_set {
                ServiceResources::Spotify(spotify_ids) => {
                    self.clone()
                        .handle_spotify_links(&ctx, new_message.clone(), spotify_ids.clone())
                        .await;
                }
                ServiceResources::Tidal(tidal_ids) => {
                    self.clone()
                        .handle_tidal_links(&ctx, new_message.clone(), tidal_ids.clone())
                        .await;
                }
                ServiceResources::Youtube(youtube_ids) => {
                    self.clone()
                        .handle_youtube_links(&ctx, new_message.clone(), youtube_ids.clone())
                        .await;
                }
            }
        }
    }
}

impl Handler {
    #[allow(clippy::too_many_lines)]
    async fn handle_youtube_links(
        &self,
        ctx: &serenity::all::Context,
        new_message: Message,
        youtube_ids: Vec<YoutubeResource>,
    ) {
        let Some(guild_id) = new_message.guild_id else {
            error!("message not in a guild");
            return;
        };

        let user_guilds = match get_user_guilds_by_guild_id_and_service(
            &self.conn,
            guild_id.to_string().as_str(),
            "youtube",
        ) {
            Ok(u) => u,
            Err(e) => {
                error!("error fetching guilds: {e}");
                return;
            }
        };

        for guild in user_guilds.clone() {
            let user = match get_user_by_user_id(&self.conn, guild.user_id) {
                Ok(u) => u,
                Err(e) => {
                    error!("Failed to get user: {e}");
                    continue;
                }
            };

            let Some(user_id) = user.id else {
                error!("user has not been created");
                continue;
            };

            let youtube_token =
                match get_oauth_token_by_user_id_and_service(&self.conn, user_id, "youtube") {
                    Ok(t) => t,
                    Err(e) => {
                        error!("Failed to get tidal token: {e}");
                        continue;
                    }
                };

            let token = isopod::client::Token {
                access_token: youtube_token.access_token,
                refresh_token: youtube_token.refresh_token,
                expiry: youtube_token.expiry_time,
                scopes: None,
            };

            let youtube_client = match youtube::init_youtube_with_token(token) {
                Ok(t) => t,
                Err(e) => {
                    error!("error initializing tidal client: {e}");
                    continue;
                }
            };

            let Some(p) = guild.playlist_id else {
                error!("playlist id not present");
                continue;
            };

            for YoutubeResource::Video(id) in youtube_ids.clone() {
                match youtube_client
                    .playlist_items_api()
                    .youtube_playlist_items_insert(
                        vec!["snippet".to_string()],
                        None,
                        None,
                        None,
                        None,
                        None,
                        None,
                        None,
                        None,
                        None,
                        None,
                        None,
                        None,
                        Some(PlaylistItem {
                            snippet: Some(Box::new(PlaylistItemSnippet {
                                playlist_id: Some(p.clone()),
                                resource_id: Some(Box::new(ResourceId {
                                    kind: Some("youtube#video".to_string()),
                                    video_id: Some(id),
                                    ..Default::default()
                                })),
                                ..Default::default()
                            })),
                            ..Default::default()
                        }),
                    )
                    .await
                {
                    Ok(_) => {
                        info!("added youtube link to playlist");
                    }
                    Err(e) => {
                        error!(
                            "failed to add youtube item to playlist {}: {}",
                            p.clone(),
                            e
                        );
                    }
                }
            }
        }
        if !user_guilds.is_empty() {
            let mills500 = std::time::Duration::from_millis(500);
            task::sleep(mills500).await;
            info!("acknowledging message");
            _ = new_message.react(&ctx, Unicode(String::from("🏮"))).await;
        }
    }
    #[allow(clippy::too_many_lines, clippy::cognitive_complexity)]
    async fn handle_tidal_links(
        &self,
        ctx: &serenity::all::Context,
        new_message: Message,
        tidal_ids: Vec<TidalResource>,
    ) {
        let Some(guild_id) = new_message.guild_id else {
            error!("message not in a guild");
            return;
        };

        let user_guilds = match get_user_guilds_by_guild_id_and_service(
            &self.conn,
            guild_id.to_string().as_str(),
            "tidal",
        ) {
            Ok(u) => u,
            Err(e) => {
                error!("error fetching guilds: {e}");
                return;
            }
        };

        let track_ids = match tidal::get_track_ids(&self.tidal_client, &tidal_ids).await {
            Ok(t) => t,
            Err(e) => {
                error!("error fetching track ids: {e}");
                return;
            }
        };

        let track_ids_payload_data: Vec<PlaylistItemsRelationshipAddOperationPayloadData> = track_ids.clone().into_iter().map(|s: String| -> PlaylistItemsRelationshipAddOperationPayloadData {
                       PlaylistItemsRelationshipAddOperationPayloadData { id: s, meta: None, r#type: models::playlist_items_relationship_add_operation_payload_data::Type::Tracks }
                    }).collect();

        info!("{} tracks to add", track_ids_payload_data.len());

        let chunked_data: Vec<&[PlaylistItemsRelationshipAddOperationPayloadData]> =
            track_ids_payload_data.chunks(20).collect();

        for guild in user_guilds.clone() {
            let user = match get_user_by_user_id(&self.conn, guild.user_id) {
                Ok(u) => u,
                Err(e) => {
                    error!("Failed to get user: {e}");
                    continue;
                }
            };

            let Some(user_id) = user.id else {
                error!("user has not been created");
                continue;
            };

            let tidal_token =
                match get_oauth_token_by_user_id_and_service(&self.conn, user_id, "tidal") {
                    Ok(t) => t,
                    Err(e) => {
                        error!("Failed to get tidal token: {e}");
                        continue;
                    }
                };

            let token = Token {
                access_token: tidal_token.access_token,
                refresh_token: tidal_token.refresh_token,
                expiry: tidal_token.expiry_time,
                scopes: None,
            };

            let tidal_client = match tidal::init_tidal_with_token(token) {
                Ok(t) => t,
                Err(e) => {
                    error!("error initializing tidal client: {e}");
                    continue;
                }
            };

            let Some(p) = guild.playlist_id else {
                error!("playlist id not present");
                continue;
            };

            for data in chunked_data.clone() {
                info!("attempting to add chunk");
                match tidal_client
                    .playlists_api()
                    .add_items_to_playlist(
                        p.as_str(),
                        None,
                        Some(PlaylistItemsRelationshipAddOperationPayload {
                            data: data.to_vec(),
                            meta: None,
                        }),
                    )
                    .await
                {
                    Ok(()) => {
                        info!("added {} items to playlist", data.len());
                    }
                    Err(e) => {
                        error!("failed to add tracks to playlist: {e}");
                    }
                }
            }
        }

        if !user_guilds.is_empty() {
            let mills500 = std::time::Duration::from_millis(500);
            task::sleep(mills500).await;
            info!("acknowledging message");
            _ = new_message.react(&ctx, Unicode(String::from("🌊"))).await;
        }
    }

    #[allow(clippy::too_many_lines, clippy::cognitive_complexity)]
    async fn handle_spotify_links(
        self,
        ctx: &serenity::all::Context,
        new_message: Message,
        spotify_ids: Vec<IdType>,
    ) {
        let Some(guild_id) = new_message.guild_id else {
            error!("message not in a guild");
            return;
        };

        let user_guilds = match get_user_guilds_by_guild_id_and_service(
            &self.conn,
            guild_id.to_string().as_str(),
            "spotify",
        ) {
            Ok(u) => u,
            Err(e) => {
                error!("error fetching guilds: {e}");
                return;
            }
        };

        let track_ids = get_track_ids(&self.spotify_client, &spotify_ids).await;

        for guild in user_guilds.clone() {
            let user = match get_user_by_user_id(&self.conn, guild.user_id) {
                Ok(u) => u,
                Err(e) => {
                    error!("Failed to get user: {e}");
                    continue;
                }
            };

            let Some(user_id) = user.id else {
                error!("user has not been created");
                continue;
            };

            let spotify_token =
                match get_oauth_token_by_user_id_and_service(&self.conn, user_id, "spotify") {
                    Ok(t) => t,
                    Err(e) => {
                        error!("Failed to get spotify token: {e}");
                        continue;
                    }
                };

            info!("token expires at: {}", spotify_token.expiry_time);

            let expires_at =
                match DateTime::parse_from_str(spotify_token.expiry_time.as_str(), "%+") {
                    Ok(t) => t.to_utc(),
                    Err(e) => {
                        error!("Error parsing expires at time: {e}");
                        continue;
                    }
                }
                .to_utc();

            let token = rspotify::Token {
                access_token: spotify_token.access_token,
                refresh_token: spotify_token.refresh_token,
                expires_at: Some(expires_at),
                expires_in: TimeDelta::default(),
                scopes: scopes!("playlist-modify-public"),
            };

            let spotify_client = match init_spotify_from_token(token) {
                Ok(c) => c,
                Err(e) => {
                    error!("error getting spotify client: {e}");
                    return;
                }
            };

            let Some(p) = guild.playlist_id else {
                error!("playlist id not present");
                continue;
            };

            let playlist_id = match PlaylistId::from_id(&p) {
                Ok(id) => id,
                Err(e) => {
                    error!("Failed to get playlist id: {e}");
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
                    error!("Failed to add tracks to playlist: {e}");
                }
            }
        }

        if !user_guilds.is_empty() {
            let mills500 = std::time::Duration::from_millis(500);
            task::sleep(mills500).await;
            info!("acknowledging message");
            _ = new_message.react(&ctx, Unicode(String::from("✅"))).await;
        }

        let album_image_urls = get_album_images(&self.spotify_client, &spotify_ids).await;

        for image in album_image_urls {
            // Send the image URL as a reply to the original message
            let _ = new_message.reply(&ctx.http, image).await;

            info!("sent track art to channel");
        }
    }
}

#[poise::command(slash_command)]
pub async fn authorize_youtube(ctx: CommandCtx<'_>) -> Result<()> {
    let discord_user_str = ctx.author().id.to_string();
    let discord_user_id = discord_user_str.as_str();

    let guild_id = match ctx.guild_id() {
        Some(id) => id.to_string(),
        None => return Err(DiscordError.into()),
    };

    let user = match first_or_create_user_by_discord_user_id(&ctx.data().conn, discord_user_id) {
        Ok(u) => u,
        Err(e) => {
            error!("error creating user: {e}");
            return Err(DiscordError.into());
        }
    };

    let Some(user_id) = user.id else {
        return Err(DiscordError.into());
    };

    let _ = match first_or_create_user_guild_by_user_id_and_guild_id(
        &ctx.data().conn,
        guild_id,
        user_id,
        "youtube",
    ) {
        Ok(u) => u,
        Err(e) => {
            error!("got error creating user guild: {e}");
            return Err(DiscordError.into());
        }
    };

    let yt_client = match init_youtube() {
        Ok(u) => u,
        Err(e) => {
            error!("got error initializing youtube client: {e}");
            return Err(DiscordError.into());
        }
    };

    let (auth_url, state) = yt_client.get_authorize_url_and_state(youtube::DEFAULT_SCOPES.to_vec());

    _ = match create_auth_request(
        &ctx.data().conn,
        state.into_secret(),
        discord_user_id,
        None,
        None,
        "youtube",
    ) {
        Ok(u) => u,
        Err(e) => {
            error!("error creating auth request: {e}");
            return Err(DiscordError.into());
        }
    };

    match ctx
        .send(
            poise::CreateReply::default()
                .content(format!(
                    "Please click this link to authorize with youtube.\n{}",
                    auth_url.as_str()
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
pub async fn authorize_spotify(ctx: CommandCtx<'_>) -> Result<()> {
    let discord_user_str = ctx.author().id.to_string();
    let discord_user_id = discord_user_str.as_str();

    let guild_id = match ctx.guild_id() {
        Some(id) => id.to_string(),
        None => return Err(DiscordError.into()),
    };

    let user = match first_or_create_user_by_discord_user_id(&ctx.data().conn, discord_user_id) {
        Ok(u) => u,
        Err(e) => {
            error!("error creating user: {e}");
            return Err(DiscordError.into());
        }
    };

    let Some(user_id) = user.id else {
        return Err(DiscordError.into());
    };

    let _ = match first_or_create_user_guild_by_user_id_and_guild_id(
        &ctx.data().conn,
        guild_id,
        user_id,
        "spotify",
    ) {
        Ok(u) => u,
        Err(e) => {
            error!("got error creating user guild: {e}");
            return Err(DiscordError.into());
        }
    };

    let mut spotify_client = match init_spotify() {
        Ok(u) => u,
        Err(e) => {
            error!("got error initializing spotify client: {e}");
            return Err(DiscordError.into());
        }
    };

    spotify_client.oauth.state = Uuid::new_v4().to_string();

    let auth_url = match spotify_client.get_authorize_url(false) {
        Ok(u) => u,
        Err(e) => {
            error!("error getting auth url: {e}");
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
            error!("error creating auth request: {e}");
            return Err(DiscordError.into());
        }
    };

    match ctx
        .send(
            poise::CreateReply::default()
                .content(format!(
                    "Please click this link to authorize with spotify.\n{auth_url}",
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
pub async fn authorize_tidal(ctx: CommandCtx<'_>) -> Result<()> {
    let discord_user_str = ctx.author().id.to_string();
    let discord_user_id = discord_user_str.as_str();

    let guild_id = match ctx.guild_id() {
        Some(id) => id.to_string(),
        None => return Err(DiscordError.into()),
    };

    let user = match first_or_create_user_by_discord_user_id(&ctx.data().conn, discord_user_id) {
        Ok(u) => u,
        Err(e) => {
            error!("error creating user: {e}");
            return Err(DiscordError.into());
        }
    };

    let Some(user_id) = user.id else {
        return Err(DiscordError.into());
    };

    let _ = match first_or_create_user_guild_by_user_id_and_guild_id(
        &ctx.data().conn,
        guild_id,
        user_id,
        "tidal",
    ) {
        Ok(u) => u,
        Err(e) => {
            error!("got error creating user guild: {e}");
            return Err(DiscordError.into());
        }
    };

    let tidal_client = match init_tidal() {
        Ok(u) => u,
        Err(e) => {
            error!("got error initializing tidal client: {e}");
            return Err(DiscordError.into());
        }
    };

    let (pkce_code, pkce_verifier) = tidal_client.generate_pkce_challenge_and_verifier();

    let (auth_url, state) =
        tidal_client.get_authorize_url_and_state(pkce_code.clone(), tidal::DEFAULT_SCOPES.to_vec());

    _ = match create_auth_request(
        &ctx.data().conn,
        state.into_secret(),
        discord_user_id,
        Some(String::from(pkce_code.as_str())),
        Some(String::from(pkce_verifier.into_secret().as_str())),
        "tidal",
    ) {
        Ok(u) => u,
        Err(e) => {
            error!("error creating auth request: {e}");
            return Err(DiscordError.into());
        }
    };

    match ctx
        .send(
            poise::CreateReply::default()
                .content(format!(
                    "Please click this link to authorize with tidal.\n{auth_url}",
                ))
                .ephemeral(true),
        )
        .await
    {
        Ok(_) => Ok(()),
        Err(e) => Err(e.into()),
    }
}

fn extract_playlist_id(service: &str, msg: &str) -> Option<String> {
    match service {
        "spotify" => spotify::extract_playlist_id(msg),
        "tidal" => tidal::extract_playlist_id(msg),
        "youtube" => youtube::extract_playlist_id(msg),
        _ => None,
    }
}

#[poise::command(slash_command)]
pub async fn register_playlist(ctx: CommandCtx<'_>, playlist_link: String) -> Result<()> {
    let service = if spotify::contains_spotify_link(playlist_link.as_str()) {
        "spotify"
    } else if tidal::contains_tidal_link(playlist_link.as_str()) {
        "tidal"
    } else if youtube::contains_youtube_link(playlist_link.as_str()) {
        "youtube"
    } else {
        return Err(DiscordError.into());
    };

    let Some(playlist_id) = extract_playlist_id(service, playlist_link.as_str()) else {
        error!("failed to parse playlist id");
        let s = ctx
            .say("Check your playlist link, we were not able to parse it.")
            .await;

        return match s {
            Ok(_) => Err(DiscordError.into()),
            Err(e) => Err(e.into()),
        };
    };

    let Some(guild_id) = ctx.guild_id() else {
        error!("Failed to get guild id");
        return Err(DiscordError.into());
    };

    let discord_id = ctx.author().id.to_string();
    let discord_user_id = discord_id.as_str();

    let user = match get_user_by_discord_user_id(&ctx.data().conn, discord_user_id) {
        Ok(u) => u,
        Err(e) => {
            error!("failed to get user: {e}");
            return Err(DiscordError.into());
        }
    };

    let Some(user_id) = user.id else {
        return Err(DiscordError.into());
    };

    match update_user_guild_playlist_id(
        &ctx.data().conn,
        guild_id.to_string(),
        user_id,
        playlist_id,
        service,
    ) {
        Ok(()) => {}
        Err(e) => {
            error!("Failed to update playlist id: {e}");
            return Err(DiscordError.into());
        }
    }

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
