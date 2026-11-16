ALTER TABLE crawls
    ADD COLUMN score int CHECK (score BETWEEN 0 AND 100),
    ADD COLUMN html_pages int NOT NULL DEFAULT 0;

CREATE TABLE findings (
    id uuid PRIMARY KEY,
    workspace_id uuid NOT NULL,
    crawl_id uuid NOT NULL REFERENCES crawls (id) ON DELETE CASCADE,
    check_id text NOT NULL,
    target text NOT NULL,
    target_kind text NOT NULL CHECK (target_kind IN ('page', 'site')),
    evidence jsonb NOT NULL DEFAULT '{}',
    created_at timestamptz NOT NULL DEFAULT now(),
    UNIQUE (crawl_id, check_id, target)
);

CREATE INDEX findings_workspace_idx ON findings (workspace_id, crawl_id);

CREATE TABLE suggestions (
    id uuid PRIMARY KEY,
    workspace_id uuid NOT NULL,
    site_id uuid NOT NULL REFERENCES sites (id) ON DELETE CASCADE,
    check_id text NOT NULL,
    first_seen timestamptz NOT NULL DEFAULT now(),
    last_seen timestamptz NOT NULL DEFAULT now(),
    UNIQUE (site_id, check_id)
);

CREATE TABLE suggestion_items (
    id uuid PRIMARY KEY,
    workspace_id uuid NOT NULL,
    suggestion_id uuid NOT NULL REFERENCES suggestions (id) ON DELETE CASCADE,
    target text NOT NULL,
    target_kind text NOT NULL CHECK (target_kind IN ('page', 'site')),
    status text NOT NULL DEFAULT 'open' CHECK (status IN ('open', 'done', 'dismissed')),
    evidence jsonb NOT NULL DEFAULT '{}',
    first_seen timestamptz NOT NULL DEFAULT now(),
    last_seen timestamptz NOT NULL DEFAULT now(),
    resolved_at timestamptz,
    UNIQUE (suggestion_id, target)
);

CREATE INDEX suggestion_items_workspace_idx ON suggestion_items (workspace_id, suggestion_id, status);
