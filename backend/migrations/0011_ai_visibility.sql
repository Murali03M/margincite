-- AI visibility: the brand to look for, the questions to ask, and what the
-- AI engines answered.
CREATE TABLE ai_settings (
    site_id uuid PRIMARY KEY REFERENCES sites (id) ON DELETE CASCADE,
    workspace_id uuid NOT NULL REFERENCES workspaces (id),
    brand_name text NOT NULL DEFAULT '',
    aliases jsonb NOT NULL DEFAULT '[]',
    competitors jsonb NOT NULL DEFAULT '[]'
);

CREATE TABLE ai_prompts (
    id uuid PRIMARY KEY,
    workspace_id uuid NOT NULL REFERENCES workspaces (id),
    site_id uuid NOT NULL REFERENCES sites (id) ON DELETE CASCADE,
    text text NOT NULL,
    created_at timestamptz NOT NULL DEFAULT now()
);
CREATE UNIQUE INDEX ai_prompts_unique_text ON ai_prompts (site_id, lower(text));

CREATE TABLE ai_runs (
    id uuid PRIMARY KEY,
    workspace_id uuid NOT NULL REFERENCES workspaces (id),
    site_id uuid NOT NULL REFERENCES sites (id) ON DELETE CASCADE,
    status text NOT NULL CHECK (status IN ('queued', 'running', 'completed', 'failed')),
    trigger text NOT NULL CHECK (trigger IN ('manual', 'scheduled')),
    engines jsonb NOT NULL DEFAULT '[]',
    answers_total int NOT NULL DEFAULT 0,
    answers_done int NOT NULL DEFAULT 0,
    error text,
    started_at timestamptz,
    finished_at timestamptz,
    created_at timestamptz NOT NULL DEFAULT now()
);
-- One queued or running run per site, even when two requests arrive together.
CREATE UNIQUE INDEX ai_runs_one_active ON ai_runs (site_id) WHERE status IN ('queued', 'running');
CREATE INDEX ai_runs_site ON ai_runs (site_id, created_at DESC);

-- prompt_id has no foreign key on purpose: deleting a question keeps what the
-- engines answered, and prompt_text says what was asked.
CREATE TABLE ai_answers (
    id uuid PRIMARY KEY,
    workspace_id uuid NOT NULL REFERENCES workspaces (id),
    run_id uuid NOT NULL REFERENCES ai_runs (id) ON DELETE CASCADE,
    site_id uuid NOT NULL REFERENCES sites (id) ON DELETE CASCADE,
    prompt_id uuid NOT NULL,
    prompt_text text NOT NULL,
    engine text NOT NULL,
    model text NOT NULL,
    answer_text text NOT NULL DEFAULT '',
    citations jsonb NOT NULL DEFAULT '[]',
    mentioned boolean NOT NULL DEFAULT false,
    cited boolean NOT NULL DEFAULT false,
    citation_rank int,
    competitor_mentions jsonb NOT NULL DEFAULT '[]',
    notes jsonb NOT NULL DEFAULT '[]',
    error text,
    input_tokens int NOT NULL DEFAULT 0,
    output_tokens int NOT NULL DEFAULT 0,
    searches int NOT NULL DEFAULT 0,
    created_at timestamptz NOT NULL DEFAULT now(),
    UNIQUE (run_id, prompt_id, engine)
);
CREATE INDEX ai_answers_site ON ai_answers (site_id, created_at);
