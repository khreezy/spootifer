ALTER TABLE "auth_requests" ADD COLUMN pkce_code_verifier TEXT;
ALTER TABLE "auth_requests" ADD COLUMN pkce_code_challenge TEXT;
ALTER TABLE "auth_requests" ADD COLUMN for_service TEXT;
