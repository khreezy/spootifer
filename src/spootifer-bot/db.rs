use std::error::Error;
use std::fmt::{Debug, Display, Formatter};
use std::sync::{Arc, Mutex};
use chrono::Utc;
use refinery::{embed_migrations, Report};
use rusqlite::{Connection, Row, Transaction};

embed_migrations!("src/spootifer-bot/migrations");

#[derive(Clone, Debug)]
pub struct User {
    pub id: Option<i64>,
    pub discord_user_id: String,
    pub deleted_at: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

pub struct SpotifyAuthToken {
    pub user_id: i64,
    pub spotify_refresh_token: String,
    pub spotify_access_token: String,
    pub spotify_expiry_time: String,
    pub spotify_token_type: String,
    pub deleted_at: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

pub struct UserGuild {
    pub user_id: i64,
    pub discord_guild_id: String,
    pub spotify_playlist_id: Option<String>,
    pub deleted_at: Option<String>,
    pub created_at: String,
    pub updated_at: String
}

struct MessageLink {
    link: String,
    message_id: String,
    guild_id: String,
    channel_id: String,
    acknowledged: bool,
    link_type: String,
}

struct SpotifyTrackAdd {
    spotify_track_id: String,
    spotify_playlist_id: String,
    message_link_id: String,
}

pub struct AuthRequest {
    pub discord_user_id: String,
    pub state: String
}

pub struct DbError;

impl Debug for DbError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "failed to acquire db lock")
    }
}

impl Display for DbError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "failed to acquire db lock")
    }
}

impl Error for DbError {}


type Result<T> = std::result::Result<T, Box<dyn Error>>;

pub(crate) fn run_migrations<'a>(mut conn: Mutex<Connection>) -> Result<Report> {
    let c = match conn.get_mut() {
        Ok(c) => c,
        Err(_) => return Err(DbError.into()),
    };

    match migrations::runner().run(c) {
        Ok(r) => Ok(r),
        Err(e) => Err(e.into()),
    }
}

pub(crate) fn get_user_guilds_by_guild_id(conn: &Arc<Mutex<Connection>>, guild_id: &str) -> Result<Vec<UserGuild>> {
    let c = match conn.try_lock() {
        Ok(c) => c,
        Err(_) => return Err(DbError.into()),
    };

    let q = c.prepare("SELECT user_id, discord_guild_id, spotify_playlist_id, deleted_at, created_at, updated_at FROM user_guilds WHERE discord_guild_id = ?");

    let r = q?.query_map([guild_id], |row: &Row| -> rusqlite::Result<UserGuild> {
        Ok(UserGuild {
            user_id: row.get(0)?,
            discord_guild_id: row.get(1)?,
            spotify_playlist_id: row.get(2)?,
            deleted_at: row.get(3)?,
            created_at: row.get(4)?,
            updated_at: row.get(5)?,
        })
    })?.into_iter().filter_map(|x: rusqlite::Result<UserGuild>| -> Option<UserGuild>{
        match x {
            Ok(u) => Some(u),
            Err(_) => None
        }
    }).collect::<Vec<UserGuild>>();

    Ok(r)
}

pub(crate) fn update_user_guild_spotify_playlist_id(conn: &Arc<Mutex<Connection>>, discord_guild_id: String, user_id: i64, playlist_id: String) -> Result<()> {
    let c = match conn.try_lock() {
        Ok(c) => c,
        Err(_) => return Err(DbError.into()),
    };

    let mut q = c.prepare("UPDATE user_guilds SET spotify_playlist_id = ? WHERE discord_guild_id = ? AND user_id = ?")?;

    let r = q.execute((playlist_id, discord_guild_id, user_id))?;

    if r > 0 {
        return Ok(())
    }

    Err(DbError.into())
}

pub(crate) fn first_or_create_user_by_discord_user_id(conn: &Arc<Mutex<Connection>>, discord_user_id: String) -> Result<User> {
    _ = match get_user_by_discord_user_id(conn, discord_user_id.clone()) {
        Ok(u) => return Ok(u),
        Err(e) => ()
    };

    let c = match conn.try_lock() {
        Ok(c) => c,
        Err(_) => return Err(DbError.into()),
    };

    let mut q = c.prepare("INSERT INTO users(discord_user_id, created_at, updated_at) VALUES(?, ?, ?)")?;

    let now = Utc::now().to_string();
    let r = q.insert((discord_user_id.clone(), now.clone(), now.clone()))?;


    Ok(User {
        id: Some(r),
        discord_user_id,
        created_at: now.clone(),
        updated_at: now.clone(),
        deleted_at: None,
    })
}

pub(crate) fn first_or_create_user_guild_by_user_id_and_guild_id(conn: &Arc<Mutex<Connection>>, guild_id: String, user_id: i64) -> Result<UserGuild> {
    _ = match get_user_guild_by_user_id_and_guild_id(conn, guild_id.clone(), user_id) {
        Ok(u) => return Ok(u),
        Err(_) => ()
    };

    let c = match conn.try_lock() {
        Ok(c) => c,
        Err(_) => return Err(DbError.into()),
    };

    let mut q = c.prepare("INSERT INTO user_guilds(user_id, discord_guild_id, created_at, updated_at) VALUES(?, ?, ?, ?)")?;

    let now = Utc::now().to_string();
    let r = q.insert((user_id, guild_id.clone(), now.clone(), now.clone()))?;

    Ok(UserGuild {
        user_id,
        discord_guild_id: guild_id.clone(),
        created_at: now.clone(),
        updated_at: now.clone(),
        deleted_at: None,
        spotify_playlist_id: None
    })
}

pub(crate) fn get_user_guild_by_guild_id(conn: &Arc<Mutex<Connection>>, guild_id: &str) -> Result<UserGuild> {
    let c = match conn.try_lock() {
        Ok(c) => c,
        Err(_) => return Err(DbError.into()),
    };

    let q = c.prepare("SELECT user_id, discord_guild_id, spotify_playlist_id, deleted_at, created_at, updated_at FROM user_guilds WHERE discord_guild_id = ?");

    let r = q?.query_row([guild_id], |row: &Row| -> rusqlite::Result<UserGuild> {
        Ok(UserGuild {
            user_id: row.get(0)?,
            discord_guild_id: row.get(1)?,
            spotify_playlist_id: row.get(2)?,
            deleted_at: row.get(3)?,
            created_at: row.get(4)?,
            updated_at: row.get(5)?,
        })
    });
    
    match r {
        Ok(r) => Ok(r),
        Err(e) => Err(e.into()),
    }
}

pub(crate) fn get_user_guild_by_user_id_and_guild_id(conn: &Arc<Mutex<Connection>>, guild_id: String, user_id: i64) -> Result<UserGuild> {
    let c = match conn.try_lock() {
        Ok(c) => c,
        Err(_) => return Err(DbError.into()),
    };

    let mut q = c.prepare("SELECT user_id, discord_guild_id, spotify_playlist_id, deleted_at, created_at, updated_at FROM user_guilds WHERE discord_guild_id = ? AND user_id = ?")?;

    let r = q.query_row((guild_id, user_id), |row: &Row| -> rusqlite::Result<UserGuild> {
        Ok(UserGuild {
            user_id: row.get(0)?,
            discord_guild_id: row.get(1)?,
            spotify_playlist_id: row.get(2)?,
            deleted_at: row.get(3)?,
            created_at: row.get(4)?,
            updated_at: row.get(5)?,
        })
    });

    match r {
        Ok(r) => Ok(r),
        Err(e) => Err(e.into()),
    }
}

pub(crate) fn get_user_by_user_id(conn: &Arc<Mutex<Connection>>, user_id: i64) -> Result<User> {
    let c = match conn.try_lock() {
        Ok(c) => c,
        Err(_) => return Err(DbError.into()),
    };

    let q = c.prepare("SELECT id, discord_user_id, deleted_at, created_at, updated_at FROM users WHERE id = ?;");

    let r =q?.query_row([user_id], |r: &Row| -> rusqlite::Result<User> {
        Ok(User {
            id: r.get(0)?,
            discord_user_id: r.get(1)?,
            deleted_at: r.get(2)?,
            created_at: r.get(3)?,
            updated_at: r.get(4)?,
        })
    });

    match r {
        Ok(u) => Ok(u),
        Err(e) => Err(e.into()),
    }
}

pub(crate) fn get_user_by_discord_user_id(conn: &Arc<Mutex<Connection>>, discord_user_id: String) -> Result<User> {
    let c = match conn.try_lock() {
        Ok(c) => c,
        Err(_) => return Err(DbError.into()),
    };

    let q = c.prepare("SELECT id, discord_user_id, deleted_at, created_at, updated_at FROM users WHERE discord_user_id = ?;");

    let r =q?.query_row([discord_user_id], |r: &Row| -> rusqlite::Result<User> {
        Ok(User {
            id: r.get(0)?,
            discord_user_id: r.get(1)?,
            deleted_at: r.get(2)?,
            created_at: r.get(3)?,
            updated_at: r.get(4)?,
        })
    });

    match r {
        Ok(u) => Ok(u),
        Err(e) => Err(e.into()),
    }
}

pub(crate) fn create_auth_request(conn: &Arc<Mutex<Connection>>, state: String, discord_user_id: String) -> Result<AuthRequest> {
    let c = match conn.try_lock() {
        Ok(c) => c,
        Err(_) => return Err(DbError.into()),
    };

    let mut q = c.prepare("INSERT INTO auth_requests(state, discord_user_id) VALUES(?,?)")?;

    _ = q.insert((state.clone(), discord_user_id.clone()))?;

    Ok(AuthRequest {
        discord_user_id,
        state
    })
}

pub(crate) fn get_auth_request_by_state(conn: &Arc<Mutex<Connection>>, discord_user_id: &str) -> Result<AuthRequest> {
    let c = match conn.try_lock() {
        Ok(c) => c,
        Err(_) => return Err(DbError.into()),
    };

    c.query_row_and_then("SELECT discord_user_id, state FROM auth_requests WHERE state = ?", [discord_user_id], |r| -> Result<AuthRequest> {
        Ok(AuthRequest {
            discord_user_id: r.get(0)?,
            state: r.get(1)?
        })
    })
}

pub(crate) fn get_spotify_auth_token_by_user_id(conn: &Arc<Mutex<Connection>>, user_id: i64) -> Result<SpotifyAuthToken> {
    let c = match conn.try_lock() {
        Ok(c) => c,
        Err(_) => return Err(DbError.into()),
    };

    let q = c.prepare("SELECT user_id, spotify_refresh_token, spotify_access_token, spotify_expiry_time, spotify_token_type, deleted_at, created_at, updated_at FROM spotify_auth_tokens WHERE user_id = ?;");

    let r = q?.query_row([user_id], |r| -> rusqlite::Result<SpotifyAuthToken> {
        Ok(SpotifyAuthToken {
            user_id: r.get(0)?,
            spotify_refresh_token: r.get(1)?,
            spotify_access_token: r.get(2)?,
            spotify_expiry_time: r.get(3)?,
            spotify_token_type: r.get(4)?,
            deleted_at: r.get(5)?,
            created_at: r.get(6)?,
            updated_at: r.get(7)?,
        })
    });

    match r {
        Ok(t) => Ok(t),
        Err(e) => Err(e.into()),
    }
}

pub(crate) fn create_user_in_transaction(conn: &Transaction, user: User) -> Result<i64> {
    let q = conn.prepare("INSERT INTO users (created_at, updated_at, discord_user_id) VALUES (?, ?, ?)");

    let r = q?.insert((user.created_at, user.updated_at, user.discord_user_id));

    match r {
        Ok(i) => Ok(i),
        Err(e) => Err(e.into()),
    }
}

pub(crate) fn create_spotify_auth_token(conn: &Transaction, token: SpotifyAuthToken) -> Result<i64> {
    let q = conn.prepare("INSERT INTO spotify_auth_tokens (user_id, spotify_refresh_token, spotify_access_token, spotify_expiry_time, spotify_token_type, created_at, updated_at) VALUES (?, ?, ?, ?, ?, ?, ?)");

    let r = q?.insert((token.user_id, token.spotify_refresh_token, token.spotify_access_token, token.spotify_expiry_time, token.spotify_token_type, token.created_at, token.updated_at));

    match r {
        Ok(i) => Ok(i),
        Err(e) => Err(e.into()),
    }
}