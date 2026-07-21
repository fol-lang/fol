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

fn generate_bash_script() -> String {
    r#"_fol() {
    local cur prev words cword
    _init_completion || return

    local -a toplevel=(work w pack p code c tool t)
    local -a work_cmds=(init new info list deps status)
    local -a pack_cmds=(fetch f sync update u upgrade)
    local -a code_cmds=(build b make run r test t check c verify emit e gen explain)
    local -a tool_cmds=(lsp format parse highlight symbols references rename complete semantic-tokens tree clean cl purge completion completions comp)
    local -a emit_cmds=(rust lowered)
    local -a tree_cmds=(generate)

    case "${words[1]}" in
        work|w)
            COMPREPLY=($(compgen -W "${work_cmds[*]}" -- "$cur"))
            return ;;
        pack|p)
            COMPREPLY=($(compgen -W "${pack_cmds[*]}" -- "$cur"))
            return ;;
        code|c)
            case "${words[2]}" in
                emit|e|gen)
                    COMPREPLY=($(compgen -W "${emit_cmds[*]}" -- "$cur"))
                    return ;;
                *)
                    COMPREPLY=($(compgen -W "${code_cmds[*]}" -- "$cur"))
                    return ;;
            esac ;;
        tool|t)
            case "${words[2]}" in
                tree)
                    COMPREPLY=($(compgen -W "${tree_cmds[*]}" -- "$cur"))
                    return ;;
                *)
                    COMPREPLY=($(compgen -W "${tool_cmds[*]}" -- "$cur"))
                    return ;;
            esac ;;
    esac

    COMPREPLY=($(compgen -W "${toplevel[*]}" -- "$cur"))
}

complete -F _fol -o default fol
"#
    .to_string()
}

fn generate_zsh_script() -> String {
    r#"#compdef fol

_fol() {
    local -a toplevel=(
        'work:Workspace management'
        'pack:Package management'
        'code:Build, run, test, check'
        'tool:Editor tools, LSP, completion'
    )
    local -a work_cmds=(init new info list deps status)
    local -a pack_cmds=(fetch update)
    local -a code_cmds=(build run test check emit explain)
    local -a tool_cmds=(lsp format parse highlight symbols references rename complete semantic-tokens tree clean completion)
    local -a emit_cmds=(rust lowered)
    local -a tree_cmds=(generate)

    _arguments -C \
        '(-h --help)'{-h,--help}'[Print help]' \
        '(-V --version)'{-V,--version}'[Print version]' \
        '1:command:->cmd' \
        '*::arg:->args'

    case $state in
        cmd)
            _describe 'command' toplevel ;;
        args)
            case ${words[1]} in
                work|w) _describe 'subcommand' work_cmds ;;
                pack|p) _describe 'subcommand' pack_cmds ;;
                code|c)
                    case ${words[2]} in
                        emit|e|gen) _describe 'subcommand' emit_cmds ;;
                        *) _describe 'subcommand' code_cmds ;;
                    esac ;;
                tool|t)
                    case ${words[2]} in
                        tree) _describe 'subcommand' tree_cmds ;;
                        *) _describe 'subcommand' tool_cmds ;;
                    esac ;;
            esac ;;
    esac
}

_fol "$@"
"#
    .to_string()
}

fn generate_fish_script() -> String {
    let mut lines = vec![
        "# Fish completions for fol".to_string(),
        "function __fish_fol_no_subcommand".to_string(),
        "    set -l cmd (commandline -opc)".to_string(),
        "    test (count $cmd) -eq 1".to_string(),
        "end".to_string(),
        "function __fish_fol_using_subcommand".to_string(),
        "    set -l cmd (commandline -opc)".to_string(),
        "    test (count $cmd) -ge 2; and contains -- $argv[1] $cmd[2]".to_string(),
        "end".to_string(),
        String::new(),
    ];

    // Top-level
    for &(name, desc) in &[
        ("work", "Workspace management"),
        ("pack", "Package management"),
        ("code", "Build, run, test, check"),
        ("tool", "Editor tools, LSP, completion"),
    ] {
        lines.push(format!(
            "complete -c fol -f -n __fish_fol_no_subcommand -a {name} -d '{desc}'"
        ));
    }
    // Aliases
    for &(alias, target) in &[("w", "work"), ("p", "pack"), ("c", "code"), ("t", "tool")] {
        lines.push(format!(
            "complete -c fol -f -n __fish_fol_no_subcommand -a {alias} -d 'Alias for {target}'"
        ));
    }
    lines.push(String::new());

    // Work subcommands
    for name in &["init", "new", "info", "list", "deps", "status"] {
        lines.push(format!(
            "complete -c fol -f -n '__fish_fol_using_subcommand work' -a {name}"
        ));
    }
    // Pack subcommands
    for &(name, aliases) in &[("fetch", "f sync"), ("update", "u upgrade")] {
        lines.push(format!(
            "complete -c fol -f -n '__fish_fol_using_subcommand pack' -a '{name} {aliases}'"
        ));
    }
    // Code subcommands
    for &(name, aliases) in &[
        ("build", "b make"),
        ("run", "r"),
        ("test", "t"),
        ("check", "c verify"),
        ("emit", "e gen"),
        ("explain", ""),
    ] {
        lines.push(format!(
            "complete -c fol -f -n '__fish_fol_using_subcommand code' -a '{name} {aliases}'"
        ));
    }
    // Tool subcommands
    for name in &[
        "lsp",
        "format",
        "parse",
        "highlight",
        "symbols",
        "references",
        "rename",
        "complete",
        "semantic-tokens",
        "tree",
        "clean",
        "completion",
    ] {
        lines.push(format!(
            "complete -c fol -f -n '__fish_fol_using_subcommand tool' -a {name}"
        ));
    }

    lines.push(String::new());
    lines.join("\n")
}

pub fn completion_command(shell: CompletionShell) -> FrontendResult<FrontendCommandResult> {
    let _ = generate_completion_script(shell)?;
    Ok(FrontendCommandResult::new(
        "completion",
        format!("generated {} completion script", shell.as_str()),
    ))
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
    Ok(FrontendCommandResult::new("_complete", matches.join("\n")))
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
    fn bash_completion_script_contains_bash_completion_shape() {
        let script = generate_bash_completion_script().unwrap();

        assert!(script.contains("_fol()"));
        assert!(script.contains("complete -F"));
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
