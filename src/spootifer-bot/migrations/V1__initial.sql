CREATE TABLE IF NOT EXISTS "users" (
    `id` integer,
    `created_at` text,
    `updated_at` text,
    `deleted_at` text,
    `discord_user_id` text,
    PRIMARY KEY (`id`)
);

CREATE INDEX IF NOT EXISTS `idx_users_deleted_at` ON `users`(`deleted_at`);

CREATE TABLE IF NOT EXISTS "user_guilds" (
    `id` integer,
    `created_at` text,
    `updated_at` text,
    `deleted_at` text,
    `user_id` integer,
    `discord_guild_id` text,
    `spotify_playlist_id` text,
    PRIMARY KEY (`id`),
    CONSTRAINT `fk_users_user_guilds` FOREIGN KEY (`user_id`) REFERENCES `users`(`id`)
 );

CREATE INDEX IF NOT EXISTS `idx_user_guilds_deleted_at` ON `user_guilds`(`deleted_at`);

CREATE TABLE IF NOT EXISTS "spotify_auth_tokens" (
    `id` integer,
    `created_at` text,
    `updated_at` text,
    `deleted_at` text,
    `user_id` integer,
    `spotify_refresh_token` text,
    `spotify_access_token` text,
    `spotify_expiry_time` text,
    `spotify_token_type` text,
    PRIMARY KEY (`id`),
    CONSTRAINT `fk_users_spotify_auth_token` FOREIGN KEY (`user_id`) REFERENCES `users`(`id`)
 );

CREATE INDEX IF NOT EXISTS `idx_spotify_auth_tokens_deleted_at` ON `spotify_auth_tokens`(`deleted_at`);

CREATE TABLE IF NOT EXISTS "message_links" (
    `id` integer,
    `created_at` datetime,
    `updated_at` datetime,
    `deleted_at` datetime,
    `link` text,
    `message_id` text,
    `guild_id` text,
    `channel_id` text,
    `acknowledged` numeric,
    `track_add_attempts` integer,
    `link_type` text,
    PRIMARY KEY (`id`)
);

CREATE INDEX IF NOT EXISTS `idx_message_links_deleted_at` ON `message_links`(`deleted_at`);