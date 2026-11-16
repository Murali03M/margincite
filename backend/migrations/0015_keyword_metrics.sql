-- Search volume, competition and cost per click for a site's keywords, from
-- the Google Ads data DataForSEO resells. Null means the source had no data.
ALTER TABLE site_keywords
    ADD COLUMN search_volume int,
    ADD COLUMN competition text,
    ADD COLUMN cpc double precision,
    ADD COLUMN metrics_checked_at timestamptz;

-- Keyword idea searches a person made, to hold them to a daily limit per
-- workspace. Each one costs money. There is no site column: the allowance
-- belongs to the workspace.
CREATE TABLE keyword_idea_requests (
    id uuid PRIMARY KEY DEFAULT gen_random_uuid(),
    workspace_id uuid NOT NULL REFERENCES workspaces (id),
    created_at timestamptz NOT NULL DEFAULT now()
);
CREATE INDEX keyword_idea_requests_workspace ON keyword_idea_requests (workspace_id, created_at);
