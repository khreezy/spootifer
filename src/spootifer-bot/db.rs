use chrono::Utc;
use refinery::{Report, embed_migrations};
use rusqlite::{Connection, Row, Transaction};
use std::error::Error;
use std::fmt::{Debug, Display, Formatter};
use std::sync::{Arc, Mutex};

embed_migrations!("src/spootifer-bot/migrations");

#[derive(Clone, Debug)]
pub struct User {
    pub id: Option<i64>,
    pub discord_user_id: String,
    pub deleted_at: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Clone)]
pub struct OAuthToken {
    pub user_id: i64,
    pub refresh_token: Option<String>,
    pub access_token: String,
    pub expiry_time: String,
    pub token_type: String,
    pub deleted_at: Option<String>,
    pub created_at: String,
    pub updated_at: String,
    pub for_service: String,
}

#[derive(Clone)]
pub struct UserGuild {
    pub user_id: i64,
    pub discord_guild_id: String,
    pub playlist_id: Option<String>,
    pub deleted_at: Option<String>,
    pub created_at: String,
    pub updated_at: String,
    pub for_service: String,
}

pub struct AuthRequest {
    pub discord_user_id: String,
    pub state: String,
    pub pkce_code_verifier: Option<String>,
    pub pkce_code_challenge: Option<String>,
    pub for_service: String,
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

pub fn run_migrations(mut conn: Mutex<Connection>) -> Result<Report> {
    let Ok(c) = conn.get_mut() else {
        return Err(DbError.into());
    };

    match migrations::runner().run(c) {
        Ok(r) => Ok(r),
        Err(e) => Err(e.into()),
    }
}

pub fn get_user_guilds_by_guild_id_and_service(
    conn: &Arc<Mutex<Connection>>,
    guild_id: &str,
    service: &str,
) -> Result<Vec<UserGuild>> {
    let Ok(c) = conn.try_lock() else {
        return Err(DbError.into());
    };

    let q = c.prepare("SELECT user_id, discord_guild_id, playlist_id, deleted_at, created_at, updated_at, for_service FROM user_guilds WHERE discord_guild_id = ? AND for_service = ?");

    let r = q?
        .query_map(
            [guild_id, service],
            |row: &Row| -> rusqlite::Result<UserGuild> {
                Ok(UserGuild {
                    user_id: row.get(0)?,
                    discord_guild_id: row.get(1)?,
                    playlist_id: row.get(2)?,
                    deleted_at: row.get(3)?,
                    created_at: row.get(4)?,
                    updated_at: row.get(5)?,
                    for_service: row.get(6)?,
                })
            },
        )?
        .filter_map(|x: rusqlite::Result<UserGuild>| -> Option<UserGuild> {
            x.map_or(None, |v| -> Option<UserGuild> { Some(v) })
        })
        .collect::<Vec<UserGuild>>();

    Ok(r)
}

pub fn update_user_guild_playlist_id(
    conn: &Arc<Mutex<Connection>>,
    discord_guild_id: String,
    user_id: i64,
    playlist_id: String,
    service: &str,
) -> Result<()> {
    let Ok(c) = conn.try_lock() else {
        return Err(DbError.into());
    };

    let mut q = c.prepare(
        "UPDATE user_guilds SET playlist_id = ? WHERE discord_guild_id = ? AND user_id = ? AND for_service = ?",
    )?;

    let r = q.execute((playlist_id, discord_guild_id, user_id, service))?;

    if r > 0 {
        return Ok(());
    }

    Err(DbError.into())
}

pub fn first_or_create_user_by_discord_user_id(
    conn: &Arc<Mutex<Connection>>,
    discord_user_id: &str,
) -> Result<User> {
    if let Ok(u) = get_user_by_discord_user_id(conn, discord_user_id) {
        return Ok(u);
    }

    let Ok(c) = conn.try_lock() else {
        return Err(DbError.into());
    };

    let mut q =
        c.prepare("INSERT INTO users(discord_user_id, created_at, updated_at) VALUES(?, ?, ?)")?;

    let now = &Utc::now().to_string();
    let r = q.insert((discord_user_id, now, now))?;

    Ok(User {
        id: Some(r),
        discord_user_id: discord_user_id.to_string(),
        created_at: now.clone(),
        updated_at: now.clone(),
        deleted_at: None,
    })
}

pub fn first_or_create_user_guild_by_user_id_and_guild_id(
    conn: &Arc<Mutex<Connection>>,
    guild_id: String,
    user_id: i64,
    service: &str,
) -> Result<UserGuild> {
    if let Ok(u) =
        get_user_guild_by_user_id_and_guild_id_and_service(conn, guild_id.clone(), user_id, service)
    {
        return Ok(u);
    }

    let Ok(c) = conn.try_lock() else {
        return Err(DbError.into());
    };

    let mut q = c.prepare("INSERT INTO user_guilds(user_id, discord_guild_id, created_at, updated_at, for_service) VALUES (?, ?, ?, ?, ?)")?;

    let now = Utc::now().to_string();
    let _ = q.insert((
        user_id,
        guild_id.to_string(),
        now.clone(),
        now.clone(),
        service,
    ))?;

    Ok(UserGuild {
        user_id,
        discord_guild_id: guild_id,
        created_at: now.clone(),
        updated_at: now,
        deleted_at: None,
        playlist_id: None,
        for_service: service.to_string(),
    })
}

pub fn get_user_guild_by_user_id_and_guild_id_and_service(
    conn: &Arc<Mutex<Connection>>,
    guild_id: String,
    user_id: i64,
    service: &str,
) -> Result<UserGuild> {
    let Ok(c) = conn.try_lock() else {
        return Err(DbError.into());
    };

    let mut q = c.prepare("SELECT user_id, discord_guild_id, playlist_id, deleted_at, created_at, updated_at, for_service FROM user_guilds WHERE discord_guild_id = ? AND user_id = ? AND for_service = ?")?;

    let r = q.query_row(
        (guild_id, user_id, service),
        |row: &Row| -> rusqlite::Result<UserGuild> {
            Ok(UserGuild {
                user_id: row.get(0)?,
                discord_guild_id: row.get(1)?,
                playlist_id: row.get(2)?,
                deleted_at: row.get(3)?,
                created_at: row.get(4)?,
                updated_at: row.get(5)?,
                for_service: row.get(6)?,
            })
        },
    );

    match r {
        Ok(r) => Ok(r),
        Err(e) => Err(e.into()),
    }
}

pub fn get_user_by_user_id(conn: &Arc<Mutex<Connection>>, user_id: i64) -> Result<User> {
    let Ok(c) = conn.try_lock() else {
        return Err(DbError.into());
    };

    let q = c.prepare(
        "SELECT id, discord_user_id, deleted_at, created_at, updated_at FROM users WHERE id = ?;",
    );

    let r = q?.query_row([user_id], |r: &Row| -> rusqlite::Result<User> {
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

pub fn get_user_by_discord_user_id(
    conn: &Arc<Mutex<Connection>>,
    discord_user_id: &str,
) -> Result<User> {
    let Ok(c) = conn.try_lock() else {
        return Err(DbError.into());
    };

    let q = c.prepare("SELECT id, discord_user_id, deleted_at, created_at, updated_at FROM users WHERE discord_user_id = ?;");

    let r = q?.query_row([discord_user_id], |r: &Row| -> rusqlite::Result<User> {
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

pub fn create_auth_request(
    conn: &Arc<Mutex<Connection>>,
    state: String,
    discord_user_id: &str,
    pkce_code_challenge: Option<String>,
    pkce_code_verifier: Option<String>,
    for_service: &str,
) -> Result<AuthRequest> {
    let Ok(c) = conn.try_lock() else {
        return Err(DbError.into());
    };

    let mut q = c.prepare("INSERT INTO auth_requests(state, discord_user_id, pkce_code_challenge, pkce_code_verifier, for_service) VALUES(?,?,?,?,?)")?;

    _ = q.insert((
        state.clone(),
        discord_user_id,
        pkce_code_challenge.clone(),
        pkce_code_verifier.clone(),
        for_service,
    ))?;

    Ok(AuthRequest {
        discord_user_id: discord_user_id.to_string(),
        state,
        pkce_code_challenge,
        pkce_code_verifier,
        for_service: String::from(for_service),
    })
}

pub fn get_auth_request_by_state(
    conn: &Arc<Mutex<Connection>>,
    discord_user_id: &str,
) -> Result<AuthRequest> {
    let Ok(c) = conn.try_lock() else {
        return Err(DbError.into());
    };

    c.query_row_and_then(
        "SELECT discord_user_id, state, pkce_code_challenge, pkce_code_verifier, for_service FROM auth_requests WHERE state = ?",
        [discord_user_id],
        |r| -> Result<AuthRequest> {
            Ok(AuthRequest {
                discord_user_id: r.get(0)?,
                state: r.get(1)?,
                pkce_code_challenge: r.get(2)?,
                pkce_code_verifier: r.get(3)?,
                for_service: r.get(4)?
            })
        },
    )
}

pub fn get_oauth_token_by_user_id_and_service(
    conn: &Arc<Mutex<Connection>>,
    user_id: i64,
    service: &str,
) -> Result<OAuthToken> {
    let Ok(c) = conn.try_lock() else {
        return Err(DbError.into());
    };

    let q = c.prepare("SELECT user_id, refresh_token, access_token, expiry_time, token_type, deleted_at, created_at, updated_at, for_service FROM oauth_tokens WHERE user_id = ? AND for_service = ?;");

    let r = q?.query_row((user_id, service), |r| -> rusqlite::Result<OAuthToken> {
        Ok(OAuthToken {
            user_id: r.get(0)?,
            refresh_token: r.get(1)?,
            access_token: r.get(2)?,
            expiry_time: r.get(3)?,
            token_type: r.get(4)?,
            deleted_at: r.get(5)?,
            created_at: r.get(6)?,
            updated_at: r.get(7)?,
            for_service: r.get(8)?,
        })
    });

    match r {
        Ok(t) => Ok(t),
        Err(e) => Err(e.into()),
    }
}

pub fn insert_oauth_token(conn: &Transaction, token: OAuthToken) -> Result<i64> {
    let q = conn.prepare("INSERT INTO oauth_tokens (user_id, refresh_token, access_token, expiry_time, token_type, created_at, updated_at, for_service) VALUES (?, ?, ?, ?, ?, ?, ?, ?)");

    let r = q?.insert((
        token.user_id,
        token.refresh_token,
        token.access_token,
        token.expiry_time,
        token.token_type,
        token.created_at,
        token.updated_at,
        token.for_service,
    ));

    match r {
        Ok(i) => Ok(i),
        Err(e) => Err(e.into()),
    }
}
