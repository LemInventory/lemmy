use activitypub_federation::config::Data;
use actix_web::web::Json;
use lemmy_api_common::{
  context::LemmyContext,
  send_activity::{ActivityChannel, SendActivityData},
  site::PurgePerson,
  utils::{is_admin, purge_local_user_images},
  SuccessResponse,
};
use lemmy_db_schema::{
  source::{
    moderator::{AdminPurgePerson, AdminPurgePersonForm},
    person::{Person, PersonUpdateForm},
  },
  traits::Crud,
};
use lemmy_db_views::structs::LocalUserView;
use lemmy_utils::error::LemmyError;

#[tracing::instrument(skip(context))]
pub async fn purge_person(
  data: Json<PurgePerson>,
  context: Data<LemmyContext>,
  local_user_view: LocalUserView,
) -> Result<Json<SuccessResponse>, LemmyError> {
  // Only let admin purge an item
  is_admin(&local_user_view)?;

  purge_local_user_images(data.person_id, &context).await.ok();

  // Clear profile data.
  Person::delete_account(&mut context.pool(), data.person_id).await?;
  // Keep person record, but mark as banned to prevent login or refetching from home instance.
  let person = Person::update(
    &mut context.pool(),
    data.person_id,
    &PersonUpdateForm {
      banned: Some(true),
      ..Default::default()
    },
  )
  .await?;

  // Mod tables
  let form = AdminPurgePersonForm {
    admin_person_id: local_user_view.person.id,
    reason: data.reason.clone(),
  };
  AdminPurgePerson::create(&mut context.pool(), &form).await?;

  ActivityChannel::submit_activity(
    SendActivityData::BanFromSite {
      moderator: local_user_view.person,
      banned_user: person,
      reason: data.reason.clone(),
      remove_data: Some(true),
      ban: true,
      expires: None,
    },
    &context,
  )
  .await?;

  Ok(Json(SuccessResponse::default()))
}
