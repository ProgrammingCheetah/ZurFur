//! Mock implementations for organization-related repository traits.

use async_trait::async_trait;
use chrono::Utc;
use domain::organization::{Organization, OrganizationError, OrganizationRepository};
use domain::organization_member::{
    OrganizationMember, OrganizationMemberError, OrganizationMemberRepository, Permissions,
};
use domain::organization_profile::{
    CommissionStatus, OrganizationProfile, OrganizationProfileError, OrganizationProfileRepository,
};
use tokio::sync::Mutex;
use uuid::Uuid;

#[derive(Default)]
pub struct MockOrgRepo {
    pub orgs: Mutex<Vec<Organization>>,
}

#[async_trait]
impl OrganizationRepository for MockOrgRepo {
    async fn create(
        &self,
        slug: &str,
        display_name: Option<&str>,
        is_personal: bool,
        created_by: Uuid,
    ) -> Result<Organization, OrganizationError> {
        let mut orgs = self.orgs.lock().await;
        if orgs.iter().any(|o| o.slug == slug) {
            return Err(OrganizationError::SlugTaken(slug.into()));
        }
        let org = Organization {
            id: Uuid::new_v4(),
            slug: slug.into(),
            display_name: display_name.map(String::from),
            is_personal,
            created_by,
            created_at: Utc::now(),
            updated_at: Utc::now(),
        };
        orgs.push(org.clone());
        Ok(org)
    }
    async fn find_by_id(&self, id: Uuid) -> Result<Option<Organization>, OrganizationError> {
        let orgs = self.orgs.lock().await;
        let org = orgs.iter().find(|o| o.id == id).cloned();
        Ok(org)
    }
    async fn find_by_slug(
        &self,
        slug: &str,
    ) -> Result<Option<Organization>, OrganizationError> {
        let orgs = self.orgs.lock().await;
        let org = orgs.iter().find(|o| o.slug == slug).cloned();
        Ok(org)
    }
    async fn find_personal_org(
        &self,
        user_id: Uuid,
    ) -> Result<Option<Organization>, OrganizationError> {
        let orgs = self.orgs.lock().await;
        let org = orgs
            .iter()
            .find(|o| o.created_by == user_id && o.is_personal)
            .cloned();
        Ok(org)
    }
    async fn update_display_name(
        &self,
        id: Uuid,
        display_name: Option<&str>,
    ) -> Result<Organization, OrganizationError> {
        let mut orgs = self.orgs.lock().await;
        let org = orgs
            .iter_mut()
            .find(|o| o.id == id)
            .ok_or(OrganizationError::NotFound)?;
        org.display_name = display_name.map(String::from);
        Ok(org.clone())
    }
    async fn soft_delete(&self, id: Uuid) -> Result<(), OrganizationError> {
        let orgs = self.orgs.lock().await;
        if orgs.iter().any(|o| o.id == id) {
            Ok(())
        } else {
            Err(OrganizationError::NotFound)
        }
    }
}

#[derive(Default)]
pub struct MockMemberRepo {
    pub members: Mutex<Vec<OrganizationMember>>,
}

#[async_trait]
impl OrganizationMemberRepository for MockMemberRepo {
    async fn add(
        &self,
        org_id: Uuid,
        user_id: Uuid,
        role: &str,
        title: Option<&str>,
        is_owner: bool,
        permissions: Permissions,
    ) -> Result<OrganizationMember, OrganizationMemberError> {
        let mut members = self.members.lock().await;
        if members
            .iter()
            .any(|m| m.org_id == org_id && m.user_id == user_id)
        {
            return Err(OrganizationMemberError::AlreadyMember);
        }
        let member = OrganizationMember {
            id: Uuid::new_v4(),
            org_id,
            user_id,
            role: role.into(),
            title: title.map(String::from),
            is_owner,
            permissions,
            joined_at: Utc::now(),
            updated_at: Utc::now(),
        };
        members.push(member.clone());
        Ok(member)
    }
    async fn find_by_org_and_user(
        &self,
        org_id: Uuid,
        user_id: Uuid,
    ) -> Result<Option<OrganizationMember>, OrganizationMemberError> {
        let members = self.members.lock().await;
        let member = members
            .iter()
            .find(|m| m.org_id == org_id && m.user_id == user_id)
            .cloned();
        Ok(member)
    }
    async fn list_by_org(
        &self,
        org_id: Uuid,
    ) -> Result<Vec<OrganizationMember>, OrganizationMemberError> {
        let members = self.members.lock().await;
        let filtered = members
            .iter()
            .filter(|m| m.org_id == org_id)
            .cloned()
            .collect();
        Ok(filtered)
    }
    async fn list_by_user(
        &self,
        user_id: Uuid,
    ) -> Result<Vec<OrganizationMember>, OrganizationMemberError> {
        let members = self.members.lock().await;
        let filtered = members
            .iter()
            .filter(|m| m.user_id == user_id)
            .cloned()
            .collect();
        Ok(filtered)
    }
    async fn update_role_and_title(
        &self,
        org_id: Uuid,
        user_id: Uuid,
        role: &str,
        title: Option<&str>,
    ) -> Result<OrganizationMember, OrganizationMemberError> {
        let mut members = self.members.lock().await;
        let member = members
            .iter_mut()
            .find(|m| m.org_id == org_id && m.user_id == user_id)
            .ok_or(OrganizationMemberError::NotFound)?;
        member.role = role.into();
        member.title = title.map(String::from);
        Ok(member.clone())
    }
    async fn update_permissions(
        &self,
        org_id: Uuid,
        user_id: Uuid,
        permissions: Permissions,
    ) -> Result<OrganizationMember, OrganizationMemberError> {
        let mut members = self.members.lock().await;
        let member = members
            .iter_mut()
            .find(|m| m.org_id == org_id && m.user_id == user_id)
            .ok_or(OrganizationMemberError::NotFound)?;
        member.permissions = permissions;
        Ok(member.clone())
    }
    async fn remove(&self, org_id: Uuid, user_id: Uuid) -> Result<(), OrganizationMemberError> {
        let mut members = self.members.lock().await;
        let len_before = members.len();
        members.retain(|m| !(m.org_id == org_id && m.user_id == user_id));
        if members.len() == len_before {
            Err(OrganizationMemberError::NotFound)
        } else {
            Ok(())
        }
    }
}

#[derive(Default)]
pub struct MockOrgProfileRepo {
    pub profiles: Mutex<Vec<OrganizationProfile>>,
}

#[async_trait]
impl OrganizationProfileRepository for MockOrgProfileRepo {
    async fn upsert(
        &self,
        org_id: Uuid,
        bio: Option<&str>,
        commission_status: CommissionStatus,
    ) -> Result<OrganizationProfile, OrganizationProfileError> {
        let mut profiles = self.profiles.lock().await;
        profiles.retain(|p| p.org_id != org_id);
        let profile = OrganizationProfile {
            org_id,
            bio: bio.map(String::from),
            commission_status,
            created_at: Utc::now(),
            updated_at: Utc::now(),
        };
        profiles.push(profile.clone());
        Ok(profile)
    }
    async fn find_by_org_id(
        &self,
        org_id: Uuid,
    ) -> Result<Option<OrganizationProfile>, OrganizationProfileError> {
        let profiles = self.profiles.lock().await;
        let profile = profiles.iter().find(|p| p.org_id == org_id).cloned();
        Ok(profile)
    }
}
