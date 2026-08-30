use crate::Context;
use crate::card;
use crate::error::{BotError, BotResult};
use crate::state::BotState;
use crate::store::Explanation;
use apod_core::ApodEntry;
use poise::{Command, CreateReply};

mod dm;
mod lookup;
mod owner;
mod search;
mod settings;

pub fn all() -> Vec<Command<BotState, BotError>> {
    vec![apod(), owner::sync()]
}

#[poise::command(
    slash_command,
    install_context = "Guild|User",
    interaction_context = "Guild|BotDm|PrivateChannel",
    subcommands(
        "lookup::today",
        "lookup::date",
        "lookup::random",
        "search::search",
        "dm::dm",
        "settings::settings",
        "settings::announce"
    )
)]
pub async fn apod(_ctx: Context<'_>) -> BotResult<()> {
    Ok(())
}

pub async fn show(ctx: Context<'_>, entry: &ApodEntry) -> BotResult<()> {
    let state = ctx.data();
    let attachment = card::thumbnail(&state.config, entry).await;
    let embed = card::embed(&state.config, entry, Explanation::Full, attachment.as_ref());

    let mut reply = CreateReply::default().embed(embed);
    if let Some(attachment) = attachment {
        reply = reply.attachment(attachment);
    }

    ctx.send(reply).await?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use poise::serenity_prelude::{InstallationContext, InteractionContext, Permissions};

    fn tree() -> Vec<Command<BotState, BotError>> {
        all()
    }

    fn find<'a>(
        commands: &'a [Command<BotState, BotError>],
        name: &str,
    ) -> &'a Command<BotState, BotError> {
        commands
            .iter()
            .find(|command| command.name == name)
            .unwrap_or_else(|| panic!("no command called '{name}'"))
    }

    #[test]
    fn a_server_sees_one_command_with_every_subcommand_hanging_off_it() {
        let tree = tree();
        let apod = find(&tree, "apod");

        let mut names: Vec<&str> = apod
            .subcommands
            .iter()
            .map(|command| command.name.as_str())
            .collect();
        names.sort_unstable();

        assert_eq!(
            names,
            [
                "announce", "date", "dm", "random", "search", "settings", "today"
            ],
            "a subcommand that is written but never listed in the parent is invisible in Discord"
        );
    }

    #[test]
    fn changing_a_server_or_rehearsing_a_post_is_for_people_who_can_manage_it() {
        let tree = tree();
        let apod = find(&tree, "apod");

        for name in ["settings", "announce"] {
            let command = find(&apod.subcommands, name);
            assert_eq!(
                command.required_permissions,
                Permissions::MANAGE_GUILD,
                "'{name}' posts to a channel or changes where it posts"
            );
            assert_eq!(
                command.default_member_permissions,
                Permissions::MANAGE_GUILD,
                "'{name}' should not even be offered to people who cannot run it"
            );
        }
    }

    #[test]
    fn the_command_reaches_both_kinds_of_install_and_every_place_they_are_used() {
        let tree = tree();
        let apod = find(&tree, "apod");

        assert_eq!(
            apod.install_context,
            Some(vec![InstallationContext::Guild, InstallationContext::User]),
            "a user install is what puts the archive in someone's own DMs"
        );
        assert_eq!(
            apod.interaction_context,
            Some(vec![
                InteractionContext::Guild,
                InteractionContext::BotDm,
                InteractionContext::PrivateChannel,
            ]),
        );

        for command in &apod.subcommands {
            assert_eq!(
                command.install_context, None,
                "Discord carries these on the top level command only, so setting one on '{}' \
                 would be a silent no-op that reads as a promise",
                command.name
            );
            assert_eq!(command.interaction_context, None, "{}", command.name);
        }
    }

    #[test]
    fn what_needs_a_server_still_says_so_where_there_is_none() {
        let tree = tree();
        let apod = find(&tree, "apod");

        assert!(
            find(&apod.subcommands, "settings").guild_only,
            "settings changes one server's channel, so a DM has nothing for it to act on"
        );

        for name in ["today", "date", "random", "search", "dm", "announce"] {
            assert!(
                !find(&apod.subcommands, name).guild_only,
                "'{name}' has something to do wherever it is run, so it must not be turned away"
            );
        }

        assert_eq!(
            find(&apod.subcommands, "announce").required_permissions,
            Permissions::MANAGE_GUILD,
            "outside a guild poise hands out every permission, so this still gates the channel \
             post while leaving the DM send open to whoever asked for it"
        );
    }

    #[test]
    fn looking_something_up_is_open_to_anyone() {
        let tree = tree();
        let apod = find(&tree, "apod");

        for name in ["today", "date", "random", "search", "dm"] {
            let command = find(&apod.subcommands, name);
            assert_eq!(command.required_permissions, Permissions::empty(), "{name}");
            assert!(!command.owners_only, "{name}");
            assert!(!command.guild_only, "{name}");
        }
    }

    #[test]
    fn sync_is_the_bootstrap_so_it_cannot_be_a_slash_command_itself() {
        let tree = tree();
        let sync = find(&tree, "sync");

        assert!(
            sync.prefix_action.is_some(),
            "registering the slash commands cannot require the slash commands to be registered"
        );
        assert!(sync.slash_action.is_none());
        assert!(sync.owners_only, "it writes to Discord on our behalf");
    }

    #[test]
    fn every_description_fits_what_discord_accepts() {
        fn check(commands: &[Command<BotState, BotError>]) {
            for command in commands
                .iter()
                .filter(|command| command.slash_action.is_some() || !command.subcommands.is_empty())
            {
                let description = command.description.as_deref().unwrap_or_default();
                assert!(
                    description.chars().count() <= 100,
                    "'{}' has a {}-character description",
                    command.name,
                    description.chars().count()
                );

                for parameter in &command.parameters {
                    let description = parameter.description.as_deref().unwrap_or_default();
                    assert!(
                        description.chars().count() <= 100,
                        "'{}' takes '{}' with a {}-character description",
                        command.name,
                        parameter.name,
                        description.chars().count()
                    );
                }

                check(&command.subcommands);
            }
        }

        check(&tree());
    }
}
