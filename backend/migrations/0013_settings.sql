-- Every credential and option a person sets up in the app. One row per
-- workspace and key. A secret is kept sealed in value_sealed and never in
-- value_plain; everything else is plain.
CREATE TABLE workspace_settings (
    workspace_id uuid NOT NULL REFERENCES workspaces (id) ON DELETE CASCADE,
    key text NOT NULL,
    value_sealed text,
    value_plain text,
    updated_at timestamptz NOT NULL DEFAULT now(),
    PRIMARY KEY (workspace_id, key),
    CHECK (value_sealed IS NULL OR value_plain IS NULL)
);

-- What the last test of a provider or source said, in plain words. balance is
-- only used by sources that report one (DataForSEO).
CREATE TABLE provider_status (
    workspace_id uuid NOT NULL REFERENCES workspaces (id) ON DELETE CASCADE,
    provider text NOT NULL,
    status text NOT NULL CHECK (status IN ('ok', 'error')),
    error text,
    balance double precision,
    checked_at timestamptz NOT NULL DEFAULT now(),
    PRIMARY KEY (workspace_id, provider)
);

-- One row per batch of paid or metered calls, for the usage screen.
CREATE TABLE usage_events (
    id uuid PRIMARY KEY DEFAULT gen_random_uuid(),
    workspace_id uuid NOT NULL REFERENCES workspaces (id) ON DELETE CASCADE,
    source text NOT NULL,
    kind text NOT NULL,
    calls int NOT NULL CHECK (calls >= 0),
    estimated_cost_usd double precision,
    created_at timestamptz NOT NULL DEFAULT now()
);
CREATE INDEX usage_events_workspace ON usage_events (workspace_id, created_at);

ALTER TABLE sites
    ADD COLUMN country text NOT NULL DEFAULT 'US',
    ADD COLUMN language text NOT NULL DEFAULT 'en',
    ADD COLUMN device text NOT NULL DEFAULT 'desktop' CHECK (device IN ('desktop', 'mobile')),
    ADD COLUMN track_rankings boolean NOT NULL DEFAULT true,
    ADD COLUMN ranking_frequency text NOT NULL DEFAULT 'weekly' CHECK (ranking_frequency IN ('daily', 'weekly'));
