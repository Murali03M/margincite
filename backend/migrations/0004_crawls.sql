CREATE TABLE crawls (
    id uuid PRIMARY KEY,
    workspace_id uuid NOT NULL,
    site_id uuid NOT NULL REFERENCES sites (id),
    status text NOT NULL CHECK (status IN ('queued', 'running', 'completed', 'failed')),
    trigger text NOT NULL CHECK (trigger IN ('manual', 'scheduled', 'first')),
    stop_reason text,
    robots_txt text,
    sitemap_found boolean NOT NULL DEFAULT false,
    llms_txt_found boolean NOT NULL DEFAULT false,
    pages_crawled int NOT NULL DEFAULT 0,
    pages_failed int NOT NULL DEFAULT 0,
    error text,
    started_at timestamptz,
    finished_at timestamptz,
    created_at timestamptz NOT NULL DEFAULT now()
);

CREATE TABLE pages (
    id uuid PRIMARY KEY,
    workspace_id uuid NOT NULL,
    crawl_id uuid NOT NULL REFERENCES crawls (id),
    url text NOT NULL,
    depth int NOT NULL,
    in_sitemap boolean NOT NULL DEFAULT false,
    status_code int,
    content_type text,
    fetch_error text,
    redirects jsonb NOT NULL DEFAULT '[]',
    response_ms int,
    bytes int,
    is_https boolean NOT NULL,
    x_robots_tag text,
    needed_javascript boolean NOT NULL DEFAULT false,
    facts jsonb,
    text text,
    created_at timestamptz NOT NULL DEFAULT now(),
    UNIQUE (crawl_id, url)
);
