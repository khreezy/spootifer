mod auth;
mod db;
mod discord;
mod spotify;
mod tidal;

use crate::auth::ExchangeToken;
use crate::db::{get_auth_request_by_state, get_user_by_discord_user_id, insert_oauth_token};
use crate::discord::Handler;
use async_std::task;
use axum::extract::Query;
use axum::routing::get;
use axum::{Router, extract::Form, extract::State};
use clap::Parser;
use http::StatusCode;
use log::{error, info};
use rsgentidal::client::TidalClient;
use rspotify::{AuthCodeSpotify, ClientCredsSpotify, Credentials};
use rusqlite::Connection;
use serde::Deserialize;
use serenity::all::GatewayIntents;
use std::env;
use std::error::Error;
use std::fmt::Debug;
use std::process::exit;
use std::sync::{Arc, Mutex};

#[derive(Parser, Debug)]
#[command(version, about, long_about = None)]
struct Args {
    // Only runs migrations if this is set.
    #[arg(short, long)]
    migrate: bool,
}

#[tokio::main]
async fn main() {
    env_logger::init();
    info!("starting spooty");
    let args = Args::parse();
    let db_path: String = env::var("DATABASE_PATH").unwrap_or_else(|_| panic!("db path not set!"));

    let discord_token: String = env::var("DISCORD_BOT_TOKEN").expect("DISCORD_BOT_TOKEN not set!");

    let mutex_conn = Mutex::new(Connection::open(db_path).unwrap());
    info!("opened db connection");

    if args.migrate {
        info!("migrating");
        db::run_migrations(mutex_conn)
            .unwrap_or_else(|e| panic!("got error performing migrations: {:?}", e));
        exit(0)
    }

    let conn = Arc::new(mutex_conn);

    let credentials: Credentials =
        Credentials::from_env().unwrap_or_else(|| panic!("Spotify credentials not set!"));

    let spotify_client: ClientCredsSpotify = ClientCredsSpotify::new(credentials);

    let handler = Handler {
        conn: conn.clone(),
        spotify_client: Arc::new(spotify_client.clone()),
    };

    let handler2 = Handler {
        conn: conn.clone(),
        spotify_client: Arc::new(spotify_client.clone()),
    };

    let framework = poise::Framework::builder()
        .options(poise::FrameworkOptions {
            commands: vec![
                discord::register_playlist(),
                discord::authorize_spotify(),
                discord::authorize_tidal(),
            ],
            ..Default::default()
        })
        .setup(move |ctx, _, framework| {
            Box::pin(async move {
                poise::builtins::register_globally(ctx, &framework.options().commands).await?;
                Ok(Arc::new(handler2))
            })
        })
        .build();

    let mut discord_client = serenity::Client::builder(&discord_token, GatewayIntents::all())
        .framework(framework)
        .event_handler(handler)
        .await
        .expect("Err creating discord client");

    task::spawn(async move {
        discord_client
            .start()
            .await
            .expect("Err connecting to discord");
        info!("started discord bot");
    });

    start_auth_server(conn.clone())
        .await
        .expect("Err starting auth server");
}

struct ServerState {
    conn: Arc<Mutex<Connection>>,
}

async fn start_auth_server(conn: Arc<Mutex<Connection>>) -> Result<(), Box<dyn Error>> {
    let server = ServerState { conn };

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
        Err(_) => panic!("oh no! couldn't start server"),
    }
}

async fn complete_auth(
    State(state): State<Arc<ServerState>>,
    code: Query<Code>,
    complete_auth_request: Form<CompleteAuthRequest>,
) -> (StatusCode, String) {
    let auth_request = match get_auth_request_by_state(
        &state.conn,
        complete_auth_request.state.clone().as_str(),
    ) {
        Ok(a) => a,
        Err(e) => {
            error!("error fetching auth request: {}", e);
            return (
                StatusCode::UNAUTHORIZED,
                "Unauthorized: auth request not found".to_string(),
            );
        }
    };

    let user = match get_user_by_discord_user_id(&state.conn, auth_request.discord_user_id.as_str())
    {
        Ok(u) => u,
        Err(e) => {
            error!("failed to get user: {}", e);
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                "Internal Server Error".to_string(),
            );
        }
    };

    let user_id = match user.id {
        Some(i) => i,
        None => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                "user id missing".to_string(),
            );
        }
    };
    let maybe_oauth_token = match auth_request.for_service.as_str() {
        "spotify" => {
            AuthCodeSpotify::exchange_token(auth_request, code.code.clone(), user_id).await
        }
        "tidal" => TidalClient::exchange_token(auth_request, code.code.clone(), user_id).await,
        _ => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                "bad service name".to_string(),
            );
        }
    };

    let auth_token = match maybe_oauth_token {
        Ok(o) => o,
        Err(e) => return (StatusCode::INTERNAL_SERVER_ERROR, format!("{:?}", e)),
    };

    let mut conn = match state.conn.try_lock() {
        Ok(c) => c,
        Err(e) => {
            error!("error locking: {}", e);
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                "Internal Server Error".to_string(),
            );
        }
    };

    let tx = match conn.transaction() {
        Ok(t) => t,
        Err(e) => {
            error!("Error opening transaction: {}", e);
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                "Internal Server Error".to_string(),
            );
        }
    };

    match insert_oauth_token(&tx, auth_token) {
        Ok(_) => {}
        Err(e) => {
            error!("Error creating auth token: {}", e);
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                "Internal Server Error".to_string(),
            );
        }
    };

    match tx.commit() {
        Ok(_) => (StatusCode::OK, "Authorized!".to_string()),
        Err(e) => {
            error!("Error committing transaction: {}", e);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                "Internal Server Error".to_string(),
            )
        }
    }
}

#[derive(Deserialize)]
struct Code {
    code: String,
}

#[derive(Deserialize)]
pub struct CompleteAuthRequest {
    state: String,
}
