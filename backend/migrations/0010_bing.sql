-- One Bing Webmaster Tools connection per site. The API key is sealed with
-- MARGINCITE_SECRET_KEY and never leaves the server.
CREATE TABLE bing_connections (
    site_id uuid PRIMARY KEY REFERENCES sites (id) ON DELETE CASCADE,
    workspace_id uuid NOT NULL REFERENCES workspaces (id),
    api_key_sealed text NOT NULL,
    site_url text NOT NULL,
    status text NOT NULL CHECK (status IN ('connected', 'needs_reconnecting')),
    error text,
    synced_at timestamptz,
    created_at timestamptz NOT NULL DEFAULT now()
);

-- The pages of the site that other sites link to, and how many links each has.
-- Replaced on every sync.
CREATE TABLE backlink_targets (
    workspace_id uuid NOT NULL REFERENCES workspaces (id),
    site_id uuid NOT NULL REFERENCES sites (id) ON DELETE CASCADE,
    target_url text NOT NULL,
    count bigint NOT NULL,
    PRIMARY KEY (site_id, target_url)
);

-- The links pointing at the most linked targets.
CREATE TABLE backlink_sources (
    workspace_id uuid NOT NULL REFERENCES workspaces (id),
    site_id uuid NOT NULL REFERENCES sites (id) ON DELETE CASCADE,
    target_url text NOT NULL,
    source_url text NOT NULL,
    anchor_text text NOT NULL DEFAULT ''
);

CREATE INDEX backlink_sources_target ON backlink_sources (site_id, target_url);
