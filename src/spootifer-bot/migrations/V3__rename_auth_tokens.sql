ALTER TABLE "spotify_auth_tokens" RENAME COLUMN "spotify_refresh_token" TO "refresh_token";
ALTER TABLE "spotify_auth_tokens"RENAME COLUMN "spotify_access_token" TO "access_token";
ALTER TABLE "spotify_auth_tokens" RENAME COLUMN "spotify_expiry_time" TO "expiry_time";
ALTER TABLE "spotify_auth_tokens" RENAME COLUMN "spotify_token_type" TO "token_type";

ALTER TABLE "spotify_auth_tokens" RENAME TO "oauth_tokens";

ALTER TABLE "oauth_tokens" ADD COLUMN "for_service" TEXT;
ALTER TABLE "oauth_tokens" ADD COLUMN "expires_in" BIGINT;
UPDATE "oauth_tokens" SET "for_service" = 'spotify';
