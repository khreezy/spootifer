ALTER TABLE "user_guilds" RENAME COLUMN "spotify_playlist_id" TO "playlist_id";
ALTER TABLE "user_guilds" ADD COLUMN "for_service" TEXT;
UPDATE "user_guilds" SET "for_service" = 'spotify';
