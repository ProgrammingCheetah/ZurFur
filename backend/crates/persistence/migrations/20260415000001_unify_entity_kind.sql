-- Expand entity_feed CHECK constraint to accept all EntityKind values.
-- Previously: ('org', 'character', 'commission', 'user')
ALTER TABLE entity_feed DROP CONSTRAINT IF EXISTS entity_feed_entity_type_check;
ALTER TABLE entity_feed ADD CONSTRAINT entity_feed_entity_type_check
    CHECK (entity_type IN (
        'user', 'org', 'character', 'commission',
        'feed', 'tag', 'feed_item', 'feed_element'
    ));

-- Expand entity_tag CHECK constraint to accept all EntityKind values.
-- Previously: ('org', 'commission', 'feed_item', 'character', 'feed_element')
ALTER TABLE entity_tag DROP CONSTRAINT IF EXISTS entity_tag_entity_type_check;
ALTER TABLE entity_tag ADD CONSTRAINT entity_tag_entity_type_check
    CHECK (entity_type IN (
        'user', 'org', 'character', 'commission',
        'feed', 'tag', 'feed_item', 'feed_element'
    ));
