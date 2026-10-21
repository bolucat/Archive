//! Parse GUI startup options and CLI commands without initializing Windows UI (§3.1/§3.8).

use snafu::Snafu;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Command {
    All,
    None,
    Allow { name: String },
    Deny { name: String },
    Syntax,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct GuiOptions {
    pub manual: bool,
    pub registry: bool,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Mode {
    Cli(Command),
    Gui(GuiOptions),
}

#[derive(Debug, PartialEq, Eq)]
pub struct Args {
    pub mode: Mode,
    pub language: Option<String>,
}

#[derive(Debug, Snafu, PartialEq, Eq)]
pub enum Error {
    #[snafu(display("A name is required for {command:?}"))]
    ParseName { command: NamedCommand },
    #[snafu(display("A language code is required after argument {index}"))]
    ParseLanguage { index: usize },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NamedCommand {
    Allow,
    Deny,
}

/// Extract global language options before interpreting the first positional argument.
/// Unknown GUI arguments retain the original tool's permissive behavior.
pub fn parse(arguments: &[String]) -> Result<Args, Error> {
    let mut language = None;
    let mut positional = Vec::new();
    let mut index = 0;
    while index < arguments.len() {
        if arguments[index].eq_ignore_ascii_case("--lang") {
            let Some(code) = arguments.get(index + 1).filter(|code| !code.is_empty()) else {
                return Err(Error::ParseLanguage { index });
            };
            language = Some(code.clone());
            index += 2;
        } else {
            positional.push(arguments[index].as_str());
            index += 1;
        }
    }
    let first = positional.first().copied().unwrap_or_default();
    let keyword = first.trim_start_matches(['-', '/']).to_ascii_lowercase();
    let command = if first.contains('?') {
        Some(Command::Syntax)
    } else {
        match keyword.as_str() {
            "all" => Some(Command::All),
            "none" => Some(Command::None),
            "allow" | "deny" => {
                let (verb, constructor): (NamedCommand, fn(String) -> Command) =
                    match keyword.as_str() {
                        "allow" => (NamedCommand::Allow, |name| Command::Allow { name }),
                        _ => (NamedCommand::Deny, |name| Command::Deny { name }),
                    };
                let name = positional[1..].join(" ").trim().to_owned();
                if name.is_empty() {
                    return Err(Error::ParseName { command: verb });
                }
                Some(constructor(name))
            }
            _ => None,
        }
    };
    Ok(Args {
        mode: command.map(Mode::Cli).unwrap_or_else(|| {
            // frmMain searches the GUI command line: manual is case-sensitive,
            // whereas -registry is case-insensitive. CLI verbs above are per-argument.
            let command_line = positional.join(" ");
            Mode::Gui(GuiOptions {
                manual: command_line.contains("manual"),
                registry: command_line.to_ascii_lowercase().contains("-registry"),
            })
        }),
        language,
    })
}

/// Find the language even when command parsing fails, so argument errors are localized.
pub fn language_hint(arguments: &[String]) -> Option<&str> {
    arguments.windows(2).rev().find_map(|pair| {
        pair[0]
            .eq_ignore_ascii_case("--lang")
            .then_some(pair[1].as_str())
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse_words(words: &[&str]) -> Result<Args, Error> {
        parse(
            &words
                .iter()
                .map(|word| (*word).to_owned())
                .collect::<Vec<_>>(),
        )
    }

    #[test]
    fn cli_keywords_are_exact_case_insensitive_and_accept_prefixes() {
        for prefix in ["", "-", "/"] {
            for (word, command) in [("AlL", Command::All), ("NONE", Command::None)] {
                assert_eq!(
                    parse_words(&[&format!("{prefix}{word}")]).unwrap().mode,
                    Mode::Cli(command)
                );
            }
            for word in ["AlLoW", "DENY"] {
                let name = "Microsoft Store".to_owned();
                let command = if word == "AlLoW" {
                    Command::Allow { name }
                } else {
                    Command::Deny { name }
                };
                assert_eq!(
                    parse_words(&[&format!("{prefix}{word}"), "Microsoft", "Store"])
                        .unwrap()
                        .mode,
                    Mode::Cli(command)
                );
            }
        }
    }

    #[test]
    fn allow_is_not_all_and_missing_names_are_errors() {
        for (command, verb) in [("allow", NamedCommand::Allow), ("deny", NamedCommand::Deny)] {
            for prefix in ["", "-", "/"] {
                assert_eq!(
                    parse_words(&[&format!("{prefix}{command}")]),
                    Err(Error::ParseName { command: verb })
                );
            }
        }
        assert!(matches!(
            parse_words(&["-allow", ""]),
            Err(Error::ParseName { .. })
        ));
        assert_eq!(
            parse_words(&["-all"]).unwrap().mode,
            Mode::Cli(Command::All)
        );
    }

    #[test]
    fn names_preserve_spaces_case_wildcards_and_remaining_arguments() {
        for (words, expected) in [
            (
                vec!["-allow", "My App", "Beta"],
                Command::Allow {
                    name: "My App Beta".into(),
                },
            ),
            (vec!["-deny", "*"], Command::Deny { name: "*".into() }),
            (vec!["-allow", "*"], Command::Allow { name: "*".into() }),
            (
                vec!["-allow", "  My App  "],
                Command::Allow {
                    name: "My App".into(),
                },
            ),
            (
                vec!["-allow", "manual", "-registry"],
                Command::Allow {
                    name: "manual -registry".into(),
                },
            ),
        ] {
            assert_eq!(parse_words(&words).unwrap().mode, Mode::Cli(expected));
        }
    }

    #[test]
    fn syntax_is_requested_only_by_first_positional_argument() {
        for first in ["?", "-?", "/?", "something?else"] {
            assert_eq!(
                parse_words(&[first]).unwrap().mode,
                Mode::Cli(Command::Syntax)
            );
        }
        assert!(matches!(
            parse_words(&["unknown", "?"]).unwrap().mode,
            Mode::Gui(_)
        ));
    }

    #[test]
    fn gui_startup_rows() {
        for (words, manual, registry) in [
            (vec![], false, false),
            (vec!["manual"], true, false),
            (vec!["-registry"], false, true),
            (vec!["unknown", "MANUAL", "-REGISTRY"], false, true),
            (vec!["unknown", "-all"], false, false),
            (vec!["allowance", "-manual", "registry"], true, false),
            (
                vec!["prefixmanualsuffix", "prefix-REGISTRYsuffix"],
                true,
                true,
            ),
        ] {
            assert_eq!(
                parse_words(&words).unwrap().mode,
                Mode::Gui(GuiOptions { manual, registry })
            );
        }
    }

    #[test]
    fn language_is_global_and_last_option_wins() {
        let args =
            parse_words(&["--lang", "ja", "-allow", "App", "Name", "--LANG", "zh-cn"]).unwrap();
        assert_eq!(args.language.as_deref(), Some("zh-cn"));
        assert_eq!(
            args.mode,
            Mode::Cli(Command::Allow {
                name: "App Name".into()
            })
        );
        assert_eq!(
            parse_words(&["--lang"]),
            Err(Error::ParseLanguage { index: 0 })
        );
        assert_eq!(
            parse_words(&["--lang", ""]),
            Err(Error::ParseLanguage { index: 0 })
        );
        assert_eq!(
            parse_words(&["--lang", "ru", "?"]).unwrap().mode,
            Mode::Cli(Command::Syntax)
        );
    }
}
