ALTER TABLE crawls DROP CONSTRAINT crawls_site_id_fkey;
ALTER TABLE crawls
    ADD CONSTRAINT crawls_site_id_fkey
    FOREIGN KEY (site_id) REFERENCES sites (id) ON DELETE CASCADE;

ALTER TABLE pages DROP CONSTRAINT pages_crawl_id_fkey;
ALTER TABLE pages
    ADD CONSTRAINT pages_crawl_id_fkey
    FOREIGN KEY (crawl_id) REFERENCES crawls (id) ON DELETE CASCADE;
