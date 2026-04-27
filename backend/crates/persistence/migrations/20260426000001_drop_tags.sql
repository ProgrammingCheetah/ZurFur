-- Drop the tag system entirely.
--
-- Tag was deferred from MVP scope; see design/GLOSSARY.md (Q10) and
-- design/SCOPE.md. When tags return, they will likely come back differently,
-- so we are removing the existing implementation rather than freezing it.
--
-- Drops in dependency order:
--   1. entity_tag (junction; FK into tag)
--   2. tag (table + indexes + trigger fall with the table)
--   3. tag_category enum
--   4. Re-tighten the entity_feed CHECK constraint to remove the now-invalid
--      'tag' entity kind value (added in 20260415000001_unify_entity_kind.sql).

DROP TABLE IF EXISTS entity_tag;
DROP TABLE IF EXISTS tag CASCADE;
DROP TYPE IF EXISTS tag_category;

ALTER TABLE entity_feed DROP CONSTRAINT IF EXISTS chk_entity_feed_entity_type;
ALTER TABLE entity_feed ADD CONSTRAINT chk_entity_feed_entity_type
    CHECK (entity_type IN (
        'user', 'org', 'character', 'commission',
        'feed', 'feed_item', 'feed_element'
    ));
