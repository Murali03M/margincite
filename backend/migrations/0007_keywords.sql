CREATE TABLE site_keywords (
    id uuid PRIMARY KEY,
    workspace_id uuid NOT NULL REFERENCES workspaces (id),
    site_id uuid NOT NULL REFERENCES sites (id) ON DELETE CASCADE,
    keyword text NOT NULL,
    target_url text,
    created_at timestamptz NOT NULL DEFAULT now()
);

CREATE UNIQUE INDEX site_keywords_site_keyword
    ON site_keywords (site_id, lower(keyword));
