use serenity::prelude::*;
use serenity::model::prelude::*;

#[allow(dead_code)]
pub async fn add_role_to_user(ctx: &Context, guild_id: GuildId, user_id: UserId, role_id: RoleId) -> Result<(), serenity::Error> {
    let member = guild_id.member(&ctx.http, user_id).await?;
    member.add_role(&ctx.http, role_id).await?;
    Ok(())
}

#[allow(dead_code)]
pub async fn remove_role_from_user(ctx: &Context, guild_id: GuildId, user_id: UserId, role_id: RoleId) -> Result<(), serenity::Error> {
    let member = guild_id.member(&ctx.http, user_id).await?;
    member.remove_role(&ctx.http, role_id).await?;
    Ok(())
}
