-- Search volume refreshes a person started, to hold them to a daily limit per
-- site. Each one is a paid request. site_id has no foreign key on purpose:
-- deleting a site must not give its allowance back.
CREATE TABLE keyword_volume_requests (
    id uuid PRIMARY KEY DEFAULT gen_random_uuid(),
    workspace_id uuid NOT NULL REFERENCES workspaces (id),
    site_id uuid NOT NULL,
    created_at timestamptz NOT NULL DEFAULT now()
);
CREATE INDEX keyword_volume_requests_site ON keyword_volume_requests (site_id, created_at);

-- Why the site's last refresh ended without storing anything. A row exists
-- only while that is the latest outcome: a refresh that is queued or that
-- stores volumes removes it. The sentence is fixed and plain, never a
-- provider's own words.
CREATE TABLE keyword_volume_failures (
    site_id uuid PRIMARY KEY REFERENCES sites (id) ON DELETE CASCADE,
    workspace_id uuid NOT NULL REFERENCES workspaces (id),
    error text NOT NULL,
    failed_at timestamptz NOT NULL DEFAULT now()
);
