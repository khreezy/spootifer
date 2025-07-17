mod db;
mod discord;
mod spotify;

use clap::Parser;
use std::{env};
use std::error::Error;
use std::fmt::{Debug};
use std::process::exit;
use axum::{Form, Router, extract::State };
use axum::routing::{get};
use chrono::{DateTime, Utc};
use log::{error, info};
use serenity::all::GatewayIntents;
use crate::discord::Handler;
use http::StatusCode;
use rspotify::{ClientCredsSpotify, Credentials};
use rspotify::clients::{BaseClient, OAuthClient};
use crate::db::{create_spotify_auth_token, get_auth_request_by_state, get_user_by_discord_user_id, SpotifyAuthToken};
use serde::Deserialize;
use crate::spotify::init_spotify;
use std::sync::{Arc, Mutex};
use async_std::{task};
use axum::extract::Query;
use rusqlite::Connection;


#[derive(Parser, Debug)]
#[command(version, about, long_about = None)]
struct Args {
    // Only runs migrations if this is set.
    #[arg(short, long)]
    migrate: bool
}

#[tokio::main]
async fn main() {
    env_logger::init();
    info!("starting spooty");
    let args = Args::parse();
    let db_path: String = env::var("DATABASE_PATH").unwrap_or_else(|_| { panic!("db path not set!") });
    
    let discord_token: String = env::var("DISCORD_BOT_TOKEN").expect("DISCORD_BOT_TOKEN not set!");
    
    let mutex_conn = Mutex::new(Connection::open(db_path).unwrap());
    info!("opened db connection");

    if args.migrate {
        info!("migrating");
        db::run_migrations(mutex_conn).unwrap_or_else(|e| { panic!("got error performing migrations: {:?}", e) });
        exit(0)
    }

    let conn = Arc::new(mutex_conn);

    let credentials: Credentials = Credentials::from_env().unwrap_or_else(|| { panic!("Spotify credentials not set!")});

    let spotify_client: ClientCredsSpotify = ClientCredsSpotify::new(credentials);

    let handler = Handler { conn: conn.clone() , spotify_client: Arc::new(spotify_client.clone()) };

    let handler2 = Handler { conn: conn.clone(), spotify_client: Arc::new(spotify_client.clone()) };

    let framework = poise::Framework::builder().options(poise::FrameworkOptions {
        commands: vec![discord::register_playlist(), discord::authorize_spotify()],
        ..Default::default()
    }).setup(move |ctx, _, framework| {
        Box::pin(async move {
            poise::builtins::register_globally(ctx, &framework.options().commands).await?;
            Ok(Arc::new(handler2))
        })
    }).build();

    let mut discord_client = serenity::Client::builder(
        &discord_token,
        GatewayIntents::all()
    ).framework(framework).event_handler(handler).await.expect("Err creating discord client");


    task::spawn(async move {
        discord_client.start().await.expect("Err connecting to discord");
        info!("started discord bot");
    });


    start_auth_server(conn.clone()).await.expect("Err starting auth server");
}

struct ServerState {
    conn: Arc<Mutex<Connection>>,
}

async fn start_auth_server(conn: Arc<Mutex<Connection>>) -> Result<(), Box<dyn Error>> {

    let server = ServerState {
        conn
    };

    let shared_state = Arc::new(server);

    let app = Router::new().route("/callback", get(complete_auth).with_state(shared_state));

    let listener = match tokio::net::TcpListener::bind("0.0.0.0:8081").await {
        Ok(l) => l,
        Err(e) => {
            panic!("Error starting server {}", e);
        }
    };

    info!("started auth listener");
    match axum::serve(listener, app).await {
        Ok(_) => Ok(()),
        Err(_) => panic!("oh no! couldn't start server")
    }
}

#[derive(Deserialize)]
struct Code {
    code: String,
}


async fn complete_auth(State(state): State<Arc<ServerState>>, code: Query<Code>, form: Form<CompleteAuthRequest>) -> (StatusCode, String){
    let auth_request = match get_auth_request_by_state(&state.conn, & form.state) {
        Ok(a) => a,
        Err(e) => {
            error ! ("error fetching auth request: {}", e);
            return (StatusCode::UNAUTHORIZED, "Unauthorized".to_string());
        }
    };

    let client = match init_spotify() {
        Ok(c) => c,
        Err(_) => {
            error ! ("Error initiating spotify client");
            return (StatusCode::INTERNAL_SERVER_ERROR, "Internal Server Error".to_string());
        }
    };

    match client.request_token(&code.code).await {
        Ok(_) => {},
        Err(e) => {
            error!("error requesting token: {:?}", e);
            return (StatusCode::INTERNAL_SERVER_ERROR, "Internal Server Error".to_string())
        }
    };

    let maybe_token = client.get_token();

    let maybe_token = match maybe_token.lock().await {
        Ok(t) => t,
        Err(_) => {
            error ! ("Error getting spotify token");
            return (StatusCode::INTERNAL_SERVER_ERROR, "Internal Server Error".to_string())
        }
    };

    let token = maybe_token.clone().expect("error getting token");

    let user = match get_user_by_discord_user_id(&state.conn, auth_request.discord_user_id.as_str()) {
        Ok(u) => u,
        Err(e) => {
            error!("failed to get user: {}", e);
            return (StatusCode::INTERNAL_SERVER_ERROR, "Internal Server Error".to_string());
        }
    };

    let user_id = match user.id {
        Some(id) => id,
        None => {
            error!("error getting id from user");
            return (StatusCode::INTERNAL_SERVER_ERROR, "Internal Server Error".to_string());
        }
    };

    let mut conn = match state.conn.try_lock() {
        Ok(c) => c,
        Err(e) => {
            error!("error locking: {}", e);
            return (StatusCode::INTERNAL_SERVER_ERROR, "Internal Server Error".to_string());
        }
    };

    let tx = match conn.transaction() {
        Ok(t) => t,
        Err(e) => {
            error ! ("Error opening transaction: {}", e);
            return (StatusCode::INTERNAL_SERVER_ERROR, "Internal Server Error".to_string())
        }
    };


    let auth_token = SpotifyAuthToken {
        user_id,
        spotify_refresh_token: match token.refresh_token { Some(t) => t, None => { error!("error getting refresh token"); return (StatusCode::INTERNAL_SERVER_ERROR, "Internal Server Error".to_string()) }},
        spotify_access_token: token.access_token,
        spotify_expiry_time: match token.expires_at { Some(t) => DateTime::to_rfc3339(&t).to_string(), None => { error!("error converting datetime!"); return (StatusCode::INTERNAL_SERVER_ERROR, "Internal Server Error".to_string()) }},
        spotify_token_type: String::from("Bearer"),
        deleted_at: None,
        created_at: Utc::now().to_string(),
        updated_at: Utc::now().to_string(),
    };


    match create_spotify_auth_token(&tx, auth_token) {
        Ok(_) => {},
        Err(e) => {
            error ! ("Error creating auth token: {}", e);
            return (StatusCode::INTERNAL_SERVER_ERROR, "Internal Server Error".to_string());
        }
    };

    match tx.commit() {
        Ok(_) => (StatusCode::OK, "Authorized!".to_string()),
        Err(e) => {
            error ! ("Error committing transaction: {}", e);
            (StatusCode::INTERNAL_SERVER_ERROR, "Internal Server Error".to_string())
        }
    }
}

#[derive(Deserialize)]
pub struct CompleteAuthRequest {
    state: String
}