use crate::{FrontendCommandResult, FrontendResult};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CompletionShell {
    Bash,
    Zsh,
    Fish,
}

impl CompletionShell {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Bash => "bash",
            Self::Zsh => "zsh",
            Self::Fish => "fish",
        }
    }
}

// ---------------------------------------------------------------------------
// Static command tree for completions
// ---------------------------------------------------------------------------

struct CmdEntry {
    name: &'static str,
    aliases: &'static [&'static str],
    subcommands: &'static [CmdEntry],
    hidden: bool,
}

static COMMAND_TREE: &[CmdEntry] = &[
    CmdEntry {
        name: "work",
        aliases: &["w"],
        hidden: false,
        subcommands: &[
            CmdEntry {
                name: "init",
                aliases: &[],
                hidden: false,
                subcommands: &[],
            },
            CmdEntry {
                name: "new",
                aliases: &[],
                hidden: false,
                subcommands: &[],
            },
            CmdEntry {
                name: "info",
                aliases: &[],
                hidden: false,
                subcommands: &[],
            },
            CmdEntry {
                name: "list",
                aliases: &[],
                hidden: false,
                subcommands: &[],
            },
            CmdEntry {
                name: "deps",
                aliases: &[],
                hidden: false,
                subcommands: &[],
            },
            CmdEntry {
                name: "status",
                aliases: &[],
                hidden: false,
                subcommands: &[],
            },
        ],
    },
    CmdEntry {
        name: "pack",
        aliases: &["p"],
        hidden: false,
        subcommands: &[
            CmdEntry {
                name: "fetch",
                aliases: &["f", "sync"],
                hidden: false,
                subcommands: &[],
            },
            CmdEntry {
                name: "update",
                aliases: &["u", "upgrade"],
                hidden: false,
                subcommands: &[],
            },
        ],
    },
    CmdEntry {
        name: "code",
        aliases: &["c"],
        hidden: false,
        subcommands: &[
            CmdEntry {
                name: "build",
                aliases: &["b", "make"],
                hidden: false,
                subcommands: &[],
            },
            CmdEntry {
                name: "run",
                aliases: &["r"],
                hidden: false,
                subcommands: &[],
            },
            CmdEntry {
                name: "test",
                aliases: &["t"],
                hidden: false,
                subcommands: &[],
            },
            CmdEntry {
                name: "check",
                aliases: &["c", "verify"],
                hidden: false,
                subcommands: &[],
            },
            CmdEntry {
                name: "emit",
                aliases: &["e", "gen"],
                hidden: false,
                subcommands: &[
                    CmdEntry {
                        name: "rust",
                        aliases: &[],
                        hidden: false,
                        subcommands: &[],
                    },
                    CmdEntry {
                        name: "lowered",
                        aliases: &[],
                        hidden: false,
                        subcommands: &[],
                    },
                ],
            },
            CmdEntry {
                name: "explain",
                aliases: &[],
                hidden: false,
                subcommands: &[],
            },
        ],
    },
    CmdEntry {
        name: "tool",
        aliases: &["t"],
        hidden: false,
        subcommands: &[
            CmdEntry {
                name: "lsp",
                aliases: &[],
                hidden: false,
                subcommands: &[],
            },
            CmdEntry {
                name: "format",
                aliases: &[],
                hidden: false,
                subcommands: &[],
            },
            CmdEntry {
                name: "parse",
                aliases: &[],
                hidden: false,
                subcommands: &[],
            },
            CmdEntry {
                name: "highlight",
                aliases: &[],
                hidden: false,
                subcommands: &[],
            },
            CmdEntry {
                name: "symbols",
                aliases: &[],
                hidden: false,
                subcommands: &[],
            },
            CmdEntry {
                name: "references",
                aliases: &[],
                hidden: false,
                subcommands: &[],
            },
            CmdEntry {
                name: "rename",
                aliases: &[],
                hidden: false,
                subcommands: &[],
            },
            CmdEntry {
                name: "complete",
                aliases: &[],
                hidden: false,
                subcommands: &[],
            },
            CmdEntry {
                name: "semantic-tokens",
                aliases: &[],
                hidden: false,
                subcommands: &[],
            },
            CmdEntry {
                name: "tree",
                aliases: &[],
                hidden: false,
                subcommands: &[CmdEntry {
                    name: "generate",
                    aliases: &[],
                    hidden: false,
                    subcommands: &[],
                }],
            },
            CmdEntry {
                name: "clean",
                aliases: &["cl", "purge"],
                hidden: false,
                subcommands: &[],
            },
            CmdEntry {
                name: "completion",
                aliases: &["completions", "comp"],
                hidden: false,
                subcommands: &[],
            },
        ],
    },
];

// ---------------------------------------------------------------------------
// Shell completion script generation
// ---------------------------------------------------------------------------

pub fn generate_completion_script(shell: CompletionShell) -> FrontendResult<String> {
    match shell {
        CompletionShell::Bash => Ok(generate_bash_script()),
        CompletionShell::Zsh => Ok(generate_zsh_script()),
        CompletionShell::Fish => Ok(generate_fish_script()),
    }
}

/// Commands the user may type, in tree order.
fn visible(entries: &'static [CmdEntry]) -> Vec<&'static CmdEntry> {
    entries.iter().filter(|entry| !entry.hidden).collect()
}

/// Space separated completion candidates: every visible name plus its aliases.
fn word_list(entries: &'static [CmdEntry]) -> String {
    let mut words = Vec::new();
    for entry in visible(entries) {
        words.push(entry.name);
        words.extend(entry.aliases.iter().copied());
    }
    words.join(" ")
}

/// `name|alias|alias` — a shell `case` pattern matching one command entry.
fn case_pattern(entry: &CmdEntry) -> String {
    std::iter::once(entry.name)
        .chain(entry.aliases.iter().copied())
        .collect::<Vec<_>>()
        .join("|")
}

/// Groups nested one level below `entries`, e.g. `code emit` and `tool tree`.
fn nested_groups(entries: &'static [CmdEntry]) -> Vec<&'static CmdEntry> {
    visible(entries)
        .into_iter()
        .filter(|entry| !entry.subcommands.is_empty())
        .collect()
}

fn group_description(name: &str) -> &'static str {
    match name {
        "work" => "Workspace management",
        "pack" => "Package management",
        "code" => "Build, run, test, check",
        "tool" => "Editor tools, LSP, completion",
        _ => "",
    }
}

/// Binaries that share this CLI surface: the toolchain manager and the compiler.
const COMPLETED_BINARIES: &[&str] = &["fol", "folc"];

fn generate_bash_script() -> String {
    let mut out = String::from("# bash completions for the FOL toolchain\n_fol() {\n");
    out.push_str("    local cur words cword\n");
    out.push_str("    if declare -F _init_completion >/dev/null 2>&1; then\n");
    out.push_str("        _init_completion || return\n");
    out.push_str("    else\n");
    out.push_str("        cur=\"${COMP_WORDS[COMP_CWORD]}\"\n");
    out.push_str("        words=(\"${COMP_WORDS[@]}\")\n");
    out.push_str("        cword=${COMP_CWORD}\n");
    out.push_str("    fi\n\n");
    out.push_str(&format!(
        "    local toplevel=\"{}\"\n\n",
        word_list(COMMAND_TREE)
    ));
    out.push_str("    if [ \"${cword}\" -le 1 ]; then\n");
    out.push_str("        COMPREPLY=($(compgen -W \"${toplevel}\" -- \"${cur}\"))\n");
    out.push_str("        return\n");
    out.push_str("    fi\n\n");
    out.push_str("    case \"${words[1]}\" in\n");
    for group in nested_groups(COMMAND_TREE) {
        out.push_str(&format!("        {})\n", case_pattern(group)));
        let nested = nested_groups(group.subcommands);
        if !nested.is_empty() {
            out.push_str("            if [ \"${cword}\" -ge 3 ]; then\n");
            out.push_str("                case \"${words[2]}\" in\n");
            for child in nested {
                out.push_str(&format!("                    {})\n", case_pattern(child)));
                out.push_str(&format!(
                    "                        COMPREPLY=($(compgen -W \"{}\" -- \"${{cur}}\"))\n",
                    word_list(child.subcommands)
                ));
                out.push_str("                        return ;;\n");
            }
            out.push_str("                esac\n");
            out.push_str("            fi\n");
        }
        out.push_str("            if [ \"${cword}\" -eq 2 ]; then\n");
        out.push_str(&format!(
            "                COMPREPLY=($(compgen -W \"{}\" -- \"${{cur}}\"))\n",
            word_list(group.subcommands)
        ));
        out.push_str("            fi\n");
        out.push_str("            return ;;\n");
    }
    out.push_str("    esac\n\n");
    out.push_str("    COMPREPLY=($(compgen -W \"${toplevel}\" -- \"${cur}\"))\n");
    out.push_str("}\n\n");
    for binary in COMPLETED_BINARIES {
        out.push_str(&format!("complete -F _fol -o default {binary}\n"));
    }
    out
}

fn zsh_array_name(path: &[&str]) -> String {
    format!("{}_cmds", path.join("_")).replace('-', "_")
}

fn generate_zsh_script() -> String {
    let mut out = format!("#compdef {}\n\n_fol() {{\n", COMPLETED_BINARIES.join(" "));
    out.push_str("    local state\n");
    out.push_str("    local -a toplevel=(\n");
    for group in visible(COMMAND_TREE) {
        let description = group_description(group.name);
        out.push_str(&format!("        '{}:{description}'\n", group.name));
        for alias in group.aliases {
            out.push_str(&format!("        '{alias}:Alias for {}'\n", group.name));
        }
    }
    out.push_str("    )\n");
    for group in nested_groups(COMMAND_TREE) {
        out.push_str(&format!(
            "    local -a {}=({})\n",
            zsh_array_name(&[group.name]),
            word_list(group.subcommands)
        ));
        for child in nested_groups(group.subcommands) {
            out.push_str(&format!(
                "    local -a {}=({})\n",
                zsh_array_name(&[group.name, child.name]),
                word_list(child.subcommands)
            ));
        }
    }
    out.push('\n');
    out.push_str("    _arguments -C \\\n");
    out.push_str("        '(-h --help)'{-h,--help}'[Print help]' \\\n");
    out.push_str("        '(-V --version)'{-V,--version}'[Print version]' \\\n");
    out.push_str("        '1:command:->cmd' \\\n");
    out.push_str("        '*::arg:->args'\n\n");
    out.push_str("    case $state in\n");
    out.push_str("        cmd)\n");
    out.push_str("            _describe 'command' toplevel ;;\n");
    out.push_str("        args)\n");
    out.push_str("            case ${words[1]} in\n");
    for group in nested_groups(COMMAND_TREE) {
        out.push_str(&format!("                {})\n", case_pattern(group)));
        let nested = nested_groups(group.subcommands);
        if !nested.is_empty() {
            out.push_str("                    if (( CURRENT > 2 )); then\n");
            out.push_str("                        case ${words[2]} in\n");
            for child in nested {
                out.push_str(&format!(
                    "                            {}) _describe 'subcommand' {}; return ;;\n",
                    case_pattern(child),
                    zsh_array_name(&[group.name, child.name])
                ));
            }
            out.push_str("                        esac\n");
            out.push_str("                    fi\n");
        }
        out.push_str(&format!(
            "                    (( CURRENT == 2 )) && _describe 'subcommand' {} ;;\n",
            zsh_array_name(&[group.name])
        ));
    }
    out.push_str("            esac ;;\n");
    out.push_str("    esac\n");
    out.push_str("}\n\n_fol \"$@\"\n");
    out
}

fn generate_fish_script() -> String {
    let mut lines = vec![
        "# Fish completions for the FOL toolchain".to_string(),
        "function __fish_fol_no_subcommand".to_string(),
        "    set -l cmd (commandline -opc)".to_string(),
        "    test (count $cmd) -eq 1".to_string(),
        "end".to_string(),
        "function __fish_fol_using_subcommand".to_string(),
        "    set -l cmd (commandline -opc)".to_string(),
        "    test (count $cmd) -ge 2; and contains -- $cmd[2] $argv".to_string(),
        "end".to_string(),
        "function __fish_fol_at_depth".to_string(),
        "    set -l cmd (commandline -opc)".to_string(),
        "    test (count $cmd) -eq $argv[1]".to_string(),
        "end".to_string(),
        String::new(),
    ];

    for binary in COMPLETED_BINARIES {
        for group in visible(COMMAND_TREE) {
            let description = group_description(group.name);
            lines.push(format!(
                "complete -c {binary} -f -n __fish_fol_no_subcommand -a '{}' -d '{description}'",
                group.name
            ));
            for alias in group.aliases {
                lines.push(format!(
                    "complete -c {binary} -f -n __fish_fol_no_subcommand -a '{alias}' -d 'Alias for {}'",
                    group.name
                ));
            }
        }
        lines.push(String::new());

        for group in nested_groups(COMMAND_TREE) {
            let group_words = case_pattern(group).replace('|', " ");
            lines.push(format!(
                "complete -c {binary} -f -n '__fish_fol_using_subcommand {group_words}; and __fish_fol_at_depth 2' -a '{}'",
                word_list(group.subcommands)
            ));
            for child in nested_groups(group.subcommands) {
                let child_words = case_pattern(child).replace('|', " ");
                lines.push(format!(
                    "complete -c {binary} -f -n '__fish_fol_using_subcommand {group_words}; and __fish_seen_subcommand_from {child_words}; and __fish_fol_at_depth 3' -a '{}'",
                    word_list(child.subcommands)
                ));
            }
        }
        lines.push(String::new());
    }

    lines.join("\n")
}

pub fn completion_command(shell: CompletionShell) -> FrontendResult<FrontendCommandResult> {
    let script = generate_completion_script(shell)?;
    Ok(FrontendCommandResult::new(
        "completion",
        format!("generated {} completion script", shell.as_str()),
    )
    .with_payload(script))
}

pub fn generate_bash_completion_script() -> FrontendResult<String> {
    generate_completion_script(CompletionShell::Bash)
}

pub fn generate_zsh_completion_script() -> FrontendResult<String> {
    generate_completion_script(CompletionShell::Zsh)
}

pub fn generate_fish_completion_script() -> FrontendResult<String> {
    generate_completion_script(CompletionShell::Fish)
}

pub fn internal_complete_command() -> FrontendResult<FrontendCommandResult> {
    internal_complete_command_with_tokens(&[])
}

pub fn internal_complete_command_with_tokens(
    tokens: &[String],
) -> FrontendResult<FrontendCommandResult> {
    let matches = internal_complete_matches(tokens);
    Ok(
        FrontendCommandResult::new("_complete", matches.join("\n"))
            .with_payload(matches.join("\n")),
    )
}

pub fn internal_complete_matches(tokens: &[String]) -> Vec<String> {
    let (path, prefix) = match tokens.split_last() {
        Some((last, rest)) => (rest, last.as_str()),
        None => (&[][..], ""),
    };
    let mut matches = Vec::new();
    collect_matches(COMMAND_TREE, path, prefix, &mut matches);

    matches.sort();
    matches.dedup();
    matches
}

fn collect_matches(entries: &[CmdEntry], path: &[String], prefix: &str, matches: &mut Vec<String>) {
    if let Some((head, tail)) = path.split_first() {
        for entry in entries {
            if entry.hidden {
                continue;
            }
            let name_match = entry.name == head;
            let alias_match = entry.aliases.contains(&head.as_str());
            if name_match || alias_match {
                collect_matches(entry.subcommands, tail, prefix, matches);
                return;
            }
        }
        return;
    }

    for entry in entries {
        if entry.hidden {
            continue;
        }
        if entry.name.starts_with(prefix) {
            matches.push(entry.name.to_string());
        }
        for &alias in entry.aliases {
            if alias.starts_with(prefix) {
                matches.push(alias.to_string());
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{
        completion_command, generate_bash_completion_script, generate_fish_completion_script,
        generate_zsh_completion_script, internal_complete_command_with_tokens,
        internal_complete_matches, CompletionShell,
    };

    #[test]
    fn completion_command_shell_reports_requested_shell() {
        let result = completion_command(CompletionShell::Bash).unwrap();

        assert_eq!(result.command, "completion");
        assert!(result.summary.contains("bash"));
    }

    #[test]
    fn internal_complete_command_has_a_stable_placeholder_surface() {
        let result = internal_complete_command_with_tokens(&["co".to_string()]).unwrap();

        assert_eq!(result.command, "_complete");
        assert!(result.summary.contains("code"));
    }

    #[test]
    fn completion_command_carries_the_generated_script_as_payload() {
        for (shell, script) in [
            (
                CompletionShell::Bash,
                generate_bash_completion_script().unwrap(),
            ),
            (
                CompletionShell::Zsh,
                generate_zsh_completion_script().unwrap(),
            ),
            (
                CompletionShell::Fish,
                generate_fish_completion_script().unwrap(),
            ),
        ] {
            let result = completion_command(shell).unwrap();
            let payload = result
                .payload
                .unwrap_or_else(|| panic!("{} completion emitted no script", shell.as_str()));

            assert!(!payload.trim().is_empty());
            assert_eq!(payload, script);
        }
    }

    #[test]
    fn internal_complete_carries_matches_as_payload() {
        let result = internal_complete_command_with_tokens(&["co".to_string()]).unwrap();

        let payload = result.payload.expect("_complete emitted no matches");
        assert!(payload.lines().any(|line| line == "code"));
    }

    #[test]
    fn completion_scripts_name_every_command_in_the_tree() {
        let scripts = [
            generate_bash_completion_script().unwrap(),
            generate_zsh_completion_script().unwrap(),
            generate_fish_completion_script().unwrap(),
        ];

        fn collect(entries: &'static [super::CmdEntry], names: &mut Vec<&'static str>) {
            for entry in super::visible(entries) {
                names.push(entry.name);
                collect(entry.subcommands, names);
            }
        }

        let mut names = Vec::new();
        collect(super::COMMAND_TREE, &mut names);
        assert!(names.len() > 20, "the command tree should be non-trivial");

        for name in names {
            for script in &scripts {
                assert!(
                    script.contains(name),
                    "generated script omits '{name}': {script}"
                );
            }
        }
    }

    #[test]
    fn bash_completion_script_contains_bash_completion_shape() {
        let script = generate_bash_completion_script().unwrap();

        assert!(script.contains("_fol()"));
        assert!(script.contains("complete -F"));
    }

    #[test]
    fn bash_completion_script_works_without_the_bash_completion_package() {
        let script = generate_bash_completion_script().unwrap();

        assert!(script.contains("declare -F _init_completion"));
        assert!(script.contains("COMP_WORDS[COMP_CWORD]"));
    }

    #[test]
    fn completion_scripts_register_both_toolchain_binaries() {
        let bash = generate_bash_completion_script().unwrap();
        let zsh = generate_zsh_completion_script().unwrap();
        let fish = generate_fish_completion_script().unwrap();

        assert!(bash.contains("complete -F _fol -o default folc"));
        assert!(zsh.contains("#compdef fol folc"));
        assert!(fish.contains("complete -c folc"));
    }

    #[test]
    fn zsh_completion_script_contains_zsh_completion_shape() {
        let script = generate_zsh_completion_script().unwrap();

        assert!(script.contains("#compdef fol"));
        assert!(script.contains("_arguments"));
    }

    #[test]
    fn fish_completion_script_contains_fish_completion_shape() {
        let script = generate_fish_completion_script().unwrap();

        assert!(script.contains("complete -c fol"));
        assert!(script.contains("__fish_fol_no_subcommand"));
    }

    #[test]
    fn code_completions_offer_the_explain_subcommand() {
        // `explain` now lives under the `code` group, not at the top level.
        let top_level = internal_complete_matches(&["e".to_string()]);
        assert!(!top_level.contains(&"explain".to_string()));

        let code_context = internal_complete_matches(&["code".to_string(), "e".to_string()]);
        assert!(code_context.contains(&"explain".to_string()));

        let bash = generate_bash_completion_script().unwrap();
        assert!(bash.contains("explain"));
        let zsh = generate_zsh_completion_script().unwrap();
        assert!(zsh.contains("explain"));
        let fish = generate_fish_completion_script().unwrap();
        assert!(fish.contains("explain"));
    }

    #[test]
    fn internal_complete_matches_filter_visible_commands_and_aliases() {
        let matches = internal_complete_matches(&["c".to_string()]);

        assert!(matches.contains(&"code".to_string()));
        assert!(matches.contains(&"c".to_string()));
    }

    #[test]
    fn internal_complete_matches_follow_subcommand_context() {
        let code_emit =
            internal_complete_matches(&["code".to_string(), "emit".to_string(), "r".to_string()]);
        let work = internal_complete_matches(&["work".to_string(), "i".to_string()]);

        assert!(code_emit.contains(&"rust".to_string()));
        assert!(work.contains(&"info".to_string()));
    }
}
