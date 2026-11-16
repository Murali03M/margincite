CREATE TABLE sites (
    id uuid PRIMARY KEY,
    workspace_id uuid NOT NULL REFERENCES workspaces (id),
    root_url text NOT NULL,
    host text NOT NULL,
    path_prefix text NOT NULL,
    display_name text NOT NULL,
    page_cap int NOT NULL DEFAULT 500,
    created_at timestamptz NOT NULL DEFAULT now(),
    UNIQUE (workspace_id, host, path_prefix)
);
