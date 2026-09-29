//! Compiled parser exposure. This is not behavioral conformance or acceptance evidence.
use clap::{Arg, Command};
use serde::Serialize;

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct Surface {
    schema_version: u8,
    evidence_kind: &'static str,
    source_head: &'static str,
    source_state: &'static str,
    frontend_presets: &'static str,
    command: CommandSurface,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct CommandSurface {
    name: String,
    aliases: Vec<String>,
    options: Vec<ArgumentSurface>,
    positionals: Vec<ArgumentSurface>,
    subcommands: Vec<CommandSurface>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct ArgumentSurface {
    id: String,
    long: Option<String>,
    long_aliases: Vec<String>,
    short: Option<char>,
    short_aliases: Vec<char>,
    value_names: Vec<String>,
    required: bool,
    global: bool,
    possible_values: Vec<String>,
}

pub(crate) fn introspect(mut command: Command) -> Surface {
    // Build expands inferred argument names, global arguments and generated help
    // in exactly the assembled parser, including feature-gated commands.
    command.build();
    Surface {
        schema_version: 1,
        evidence_kind: "parser-surface",
        source_head: env!("BRIT_GIT_SHA"),
        source_state: env!("BRIT_SOURCE_STATE"),
        frontend_presets: env!("BRIT_FEATURES"),
        command: command_surface(&command),
    }
}

fn command_surface(command: &Command) -> CommandSurface {
    CommandSurface {
        name: command.get_name().to_owned(),
        aliases: command.get_all_aliases().map(str::to_owned).collect(),
        options: command
            .get_arguments()
            .filter(|arg| !arg.is_positional())
            .map(argument_surface)
            .collect(),
        positionals: command.get_positionals().map(argument_surface).collect(),
        subcommands: command.get_subcommands().map(command_surface).collect(),
    }
}

fn argument_surface(arg: &Arg) -> ArgumentSurface {
    ArgumentSurface {
        id: arg.get_id().as_str().to_owned(),
        long: arg.get_long().map(str::to_owned),
        long_aliases: arg
            .get_all_aliases()
            .unwrap_or_default()
            .into_iter()
            .map(str::to_owned)
            .collect(),
        short: arg.get_short(),
        short_aliases: arg.get_all_short_aliases().unwrap_or_default(),
        value_names: arg
            .get_value_names()
            .unwrap_or_default()
            .iter()
            .map(ToString::to_string)
            .collect(),
        required: arg.is_required_set(),
        global: arg.is_global_set(),
        possible_values: arg
            .get_possible_values()
            .iter()
            .map(|value| value.get_name().to_owned())
            .collect(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn parser_aliases_values_positionals_and_inherited_options_are_exposed() {
        let command = Command::new("fixture")
            .arg(
                Arg::new("mode")
                    .long("mode")
                    .alias("hidden-mode")
                    .visible_alias("public-mode")
                    .short('m')
                    .short_alias('M')
                    .value_name("MODE")
                    .value_parser(["fast", "safe"])
                    .global(true),
            )
            .subcommand(
                Command::new("operation")
                    .alias("hidden-op")
                    .visible_alias("op")
                    .arg(Arg::new("target").required(true)),
            );
        let value = serde_json::to_value(introspect(command)).unwrap();
        assert_eq!(value["evidenceKind"], "parser-surface");
        let child = &value["command"]["subcommands"][0];
        assert_eq!(child["aliases"], serde_json::json!(["hidden-op", "op"]));
        assert_eq!(child["positionals"][0]["required"], true);
        let inherited = child["options"]
            .as_array()
            .unwrap()
            .iter()
            .find(|arg| arg["id"] == "mode")
            .unwrap();
        assert_eq!(inherited["global"], true);
        assert_eq!(
            inherited["longAliases"],
            serde_json::json!(["hidden-mode", "public-mode"])
        );
        assert_eq!(inherited["shortAliases"], serde_json::json!(["M"]));
        assert_eq!(inherited["valueNames"], serde_json::json!(["MODE"]));
        assert_eq!(inherited["possibleValues"], serde_json::json!(["fast", "safe"]));
    }
}
