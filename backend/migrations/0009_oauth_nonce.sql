-- Binds a sign-in to the browser that started it: only the SHA-256 hash of a
-- random nonce is kept here, the nonce itself lives in that browser's cookie.
-- Rows without a hash (made before this) can never be completed.
ALTER TABLE oauth_states ADD COLUMN nonce_hash text;
