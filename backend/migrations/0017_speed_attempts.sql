-- When a page was last tested, whether or not the test worked. checked_at
-- keeps meaning "when the stored numbers were measured".
ALTER TABLE speed_results ADD COLUMN attempted_at timestamptz NOT NULL DEFAULT now();
UPDATE speed_results SET attempted_at = checked_at;
