-- Keyword rankings and Google AI Overviews, from DataForSEO search results.
-- One run checks every keyword of a site; each keyword's result is one row.
CREATE TABLE rank_runs (
    id uuid PRIMARY KEY,
    workspace_id uuid NOT NULL REFERENCES workspaces (id),
    site_id uuid NOT NULL REFERENCES sites (id) ON DELETE CASCADE,
    status text NOT NULL CHECK (status IN ('queued', 'running', 'completed', 'failed')),
    trigger text NOT NULL CHECK (trigger IN ('manual', 'scheduled')),
    keywords_total int NOT NULL DEFAULT 0,
    keywords_done int NOT NULL DEFAULT 0,
    keywords_failed int NOT NULL DEFAULT 0,
    error text,
    started_at timestamptz,
    finished_at timestamptz,
    created_at timestamptz NOT NULL DEFAULT now()
);
-- One queued or running run per site, even when two requests arrive together.
CREATE UNIQUE INDEX rank_runs_one_active ON rank_runs (site_id) WHERE status IN ('queued', 'running');
CREATE INDEX rank_runs_site ON rank_runs (site_id, created_at DESC);

-- A keyword that could not be checked has no row: a missing row means "not
-- checked", and a row with a null position means "not in the top 100".
-- keyword_id has no foreign key on purpose: the history outlives a deleted
-- keyword and is matched to a re-added one by its text.
CREATE TABLE rank_checks (
    id uuid PRIMARY KEY,
    workspace_id uuid NOT NULL REFERENCES workspaces (id),
    site_id uuid NOT NULL REFERENCES sites (id) ON DELETE CASCADE,
    run_id uuid REFERENCES rank_runs (id) ON DELETE CASCADE,
    keyword_id uuid NOT NULL,
    keyword text NOT NULL,
    country text NOT NULL,
    language text NOT NULL,
    device text NOT NULL,
    position int CHECK (position >= 1),
    url text,
    ai_overview text NOT NULL CHECK (ai_overview IN ('cited', 'present', 'absent', 'unknown')),
    ai_cited_domains jsonb NOT NULL DEFAULT '[]',
    ai_your_urls jsonb NOT NULL DEFAULT '[]',
    top jsonb NOT NULL DEFAULT '[]',
    checked_at timestamptz NOT NULL DEFAULT now()
);
CREATE INDEX rank_checks_history
    ON rank_checks (site_id, country, language, device, lower(keyword), checked_at DESC);
-- A retried run does not store a keyword twice.
CREATE UNIQUE INDEX rank_checks_once_per_run ON rank_checks (run_id, lower(keyword))
    WHERE run_id IS NOT NULL;
