-- The latest PageSpeed Insights result for each tested page of a site. A page
-- that could not be tested keeps its last good numbers and gains an error.
CREATE TABLE speed_results (
    workspace_id uuid NOT NULL REFERENCES workspaces (id),
    site_id uuid NOT NULL REFERENCES sites (id) ON DELETE CASCADE,
    url text NOT NULL,
    strategy text NOT NULL CHECK (strategy IN ('mobile')),
    performance int CHECK (performance BETWEEN 0 AND 100),
    lcp_ms int,
    cls double precision,
    inp_ms int,
    tbt_ms int,
    field_data boolean NOT NULL DEFAULT false,
    error text,
    checked_at timestamptz NOT NULL DEFAULT now(),
    PRIMARY KEY (site_id, url, strategy)
);

-- Checks a person started by hand, to hold them to a daily limit. site_id has
-- no foreign key on purpose: deleting a site must not give its allowance back.
CREATE TABLE speed_manual_checks (
    id uuid PRIMARY KEY DEFAULT gen_random_uuid(),
    workspace_id uuid NOT NULL REFERENCES workspaces (id),
    site_id uuid NOT NULL,
    created_at timestamptz NOT NULL DEFAULT now()
);
CREATE INDEX speed_manual_checks_site ON speed_manual_checks (site_id, created_at);
