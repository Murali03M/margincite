CREATE TABLE jobs (
    id uuid PRIMARY KEY,
    kind text NOT NULL,
    payload jsonb NOT NULL,
    status text NOT NULL CHECK (status IN ('queued', 'running', 'done', 'failed')),
    run_at timestamptz NOT NULL,
    attempts int NOT NULL DEFAULT 0,
    locked_at timestamptz,
    locked_by text,
    last_error text,
    created_at timestamptz NOT NULL DEFAULT now()
);

CREATE INDEX jobs_due ON jobs (run_at, created_at) WHERE status = 'queued';
