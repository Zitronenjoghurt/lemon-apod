use crate::Context;
use crate::error::BotResult;

#[poise::command(prefix_command, owners_only, hide_in_help)]
pub async fn sync(ctx: Context<'_>) -> BotResult<()> {
    poise::builtins::register_application_commands_buttons(ctx).await?;
    Ok(())
}
