-- Role enum refactor: replace is_owner boolean + free-text role with
-- a constrained role enum. Ownership is now derived from role = 'owner'.
--
-- ARCHITECTURE DECISIONS:
--   Roles are administrative positions (owner/admin/mod/member), not cosmetic.
--   Cosmetic identifiers like "Artist" or "Furry Illustrator" belong in the
--   title column. The is_owner boolean is removed — ownership is derived from
--   role = 'owner' via OrganizationMember::is_owner() in the domain layer.

-- 1. Ensure existing owner members have role='owner'
UPDATE organization_members SET role = 'owner' WHERE is_owner = true;

-- 2. Add CHECK constraint matching the Role enum
ALTER TABLE organization_members
    ADD CONSTRAINT chk_organization_members_role
    CHECK (role IN ('owner', 'admin', 'mod', 'member'));

-- 3. Drop is_owner (now derived from role)
ALTER TABLE organization_members DROP COLUMN is_owner;

-- 4. Remove "artist" default role (roles are administrative, not cosmetic)
DELETE FROM default_roles WHERE name = 'artist';

-- 5. Fix hierarchy levels after removing artist
UPDATE default_roles SET hierarchy_level = 2 WHERE name = 'mod';
UPDATE default_roles SET hierarchy_level = 3 WHERE name = 'member';
