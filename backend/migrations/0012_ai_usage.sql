-- What a workspace spent on paid AI calls: one row per manual run started and
-- per question-suggestion request. site_id has no foreign key on purpose:
-- deleting a site must not give its allowance back, so these rows outlive it.
CREATE TABLE ai_usage (
    id uuid PRIMARY KEY DEFAULT gen_random_uuid(),
    workspace_id uuid NOT NULL REFERENCES workspaces (id),
    site_id uuid NOT NULL,
    kind text NOT NULL CHECK (kind IN ('manual_run', 'suggest')),
    created_at timestamptz NOT NULL DEFAULT now()
);
CREATE INDEX ai_usage_workspace ON ai_usage (workspace_id, kind, created_at);
CREATE INDEX ai_usage_site ON ai_usage (site_id, kind, created_at);

-- Runs already started today still count against today's allowance.
INSERT INTO ai_usage (workspace_id, site_id, kind, created_at)
SELECT workspace_id, site_id, 'manual_run', created_at FROM ai_runs WHERE trigger = 'manual';
