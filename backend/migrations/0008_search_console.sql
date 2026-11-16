-- Sign-in attempts that have left for a provider and not yet come back.
-- Single use, and only honoured for 10 minutes.
CREATE TABLE oauth_states (
    id uuid PRIMARY KEY,
    workspace_id uuid NOT NULL REFERENCES workspaces (id),
    site_id uuid NOT NULL REFERENCES sites (id) ON DELETE CASCADE,
    provider text NOT NULL,
    created_at timestamptz NOT NULL DEFAULT now()
);

CREATE INDEX oauth_states_created ON oauth_states (created_at);

-- One Search Console connection per site. The refresh token is sealed with
-- MARGINCITE_SECRET_KEY and never leaves the server.
CREATE TABLE google_connections (
    site_id uuid PRIMARY KEY REFERENCES sites (id) ON DELETE CASCADE,
    workspace_id uuid NOT NULL REFERENCES workspaces (id),
    refresh_token_sealed text NOT NULL,
    property text,
    status text NOT NULL CHECK (status IN ('choose_property', 'connected', 'needs_reconnecting')),
    error text,
    synced_at timestamptz,
    created_at timestamptz NOT NULL DEFAULT now()
);

-- Query and page rows for the last 28 days, replaced on every sync.
CREATE TABLE search_rows (
    id bigserial PRIMARY KEY,
    workspace_id uuid NOT NULL REFERENCES workspaces (id),
    site_id uuid NOT NULL REFERENCES sites (id) ON DELETE CASCADE,
    query text NOT NULL,
    page text NOT NULL,
    clicks bigint NOT NULL,
    impressions bigint NOT NULL,
    ctr double precision NOT NULL,
    position double precision NOT NULL
);

CREATE INDEX search_rows_site ON search_rows (site_id, page);

-- Totals per day for the last 90 days.
CREATE TABLE search_daily (
    workspace_id uuid NOT NULL REFERENCES workspaces (id),
    site_id uuid NOT NULL REFERENCES sites (id) ON DELETE CASCADE,
    date date NOT NULL,
    clicks bigint NOT NULL,
    impressions bigint NOT NULL,
    PRIMARY KEY (site_id, date)
);

-- Totals for the 28 days before the rows above, to show the change.
CREATE TABLE search_previous (
    site_id uuid PRIMARY KEY REFERENCES sites (id) ON DELETE CASCADE,
    workspace_id uuid NOT NULL REFERENCES workspaces (id),
    clicks bigint NOT NULL,
    impressions bigint NOT NULL,
    keywords bigint NOT NULL
);
