use domain::{
    datetime::DateTimeUtc,
    elements::{
        commission::{CommissionId, ElementId, NewSlot, SlotTitle, SurfaceAddress, TabId},
        user::UserId,
    },
    ports::UnitOfWork,
};

use crate::{
    commission::{CommissionPorts, CommissionResult, slots::Slots},
    ports::WithPorts,
    transaction,
};

pub struct SlotBody {
    pub tab: TabId,
    pub surface: SurfaceAddress,
    pub title: SlotTitle,
    pub notes: Option<String>,
}
pub struct Command {
    pub user_id: UserId,
    pub commission_id: CommissionId,
    pub slots: Vec<SlotBody>,
}
pub struct Output {
    pub slot_ids: Vec<ElementId>,
}

impl Slots<'_> {
    pub async fn declare(&self, cmd: Command, now: DateTimeUtc) -> CommissionResult<Output> {
        let Command {
            user_id,
            commission_id,
            slots,
        } = cmd;
        let slots: Vec<NewSlot> = slots
            .into_iter()
            .map(|s| {
                NewSlot::contributed_at(
                    commission_id,
                    s.surface.clone(),
                    s.title.clone(),
                    s.notes.clone(),
                    user_id.clone(),
                    now,
                )
            })
            .collect();

        let slot_ids: Vec<ElementId> = slots.iter().map(|s| s.id).collect();
        let mut uow = self.ports().database.begin().await?;
        uow.commissions().declare_slots(&slots).await?;
        uow.commit().await?;
        Ok(Output { slot_ids })
    }
}
