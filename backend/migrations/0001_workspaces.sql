CREATE TABLE workspaces (
    id uuid PRIMARY KEY,
    name text NOT NULL,
    created_at timestamptz NOT NULL DEFAULT now()
);

CREATE TABLE users (
    id uuid PRIMARY KEY,
    workspace_id uuid NOT NULL REFERENCES workspaces (id),
    email text NOT NULL UNIQUE,
    created_at timestamptz NOT NULL DEFAULT now()
);
