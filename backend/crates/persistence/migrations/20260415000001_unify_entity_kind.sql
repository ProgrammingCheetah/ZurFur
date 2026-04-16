-- Expand entity_feed CHECK constraint to accept all EntityKind values.
-- Previously: ('org', 'character', 'commission', 'user')
-- Original constraint named chk_entity_feeds_entity_type (created on entity_feeds before table rename).
ALTER TABLE entity_feed DROP CONSTRAINT IF EXISTS chk_entity_feeds_entity_type;
ALTER TABLE entity_feed ADD CONSTRAINT chk_entity_feed_entity_type
    CHECK (entity_type IN (
        'user', 'org', 'character', 'commission',
        'feed', 'tag', 'feed_item', 'feed_element'
    ));

-- Expand entity_tag CHECK constraint to accept all EntityKind values.
-- Previously: ('org', 'commission', 'feed_item', 'character', 'feed_element')
ALTER TABLE entity_tag DROP CONSTRAINT IF EXISTS chk_entity_tag_entity_type;
ALTER TABLE entity_tag ADD CONSTRAINT chk_entity_tag_entity_type
    CHECK (entity_type IN (
        'user', 'org', 'character', 'commission',
        'feed', 'tag', 'feed_item', 'feed_element'
    ));
