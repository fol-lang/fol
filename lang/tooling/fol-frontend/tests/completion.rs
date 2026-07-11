use fol_frontend::{
    generate_bash_completion_script, generate_fish_completion_script,
    generate_zsh_completion_script, internal_complete_matches, run_command_from_args,
};

#[test]
fn completion_scripts_are_generated_through_public_api() {
    let bash = generate_bash_completion_script().expect("bash completion should generate");
    let zsh = generate_zsh_completion_script().expect("zsh completion should generate");
    let fish = generate_fish_completion_script().expect("fish completion should generate");

    assert!(bash.contains("_fol()"));
    assert!(zsh.contains("#compdef fol"));
    assert!(fish.contains("complete -c fol"));
}

#[test]
fn internal_completion_matches_follow_command_context_through_public_api() {
    let emit = internal_complete_matches(&["code".to_string(), "emit".to_string(), "r".to_string()]);
    let work = internal_complete_matches(&["work".to_string(), "l".to_string()]);

    assert!(emit.contains(&"rust".to_string()));
    assert!(work.contains(&"list".to_string()));
}

#[test]
fn completion_commands_dispatch_through_public_frontend_entrypoints() {
    let (_, completion) = run_command_from_args(["fol", "tool", "completion", "bash"])
        .expect("completion command should run");
    let (_, complete) = run_command_from_args(["fol", "_complete", "code", "emit", "ru"])
        .expect("_complete should run");

    assert_eq!(completion.command, "completion");
    assert!(completion.summary.contains("bash"));
    assert_eq!(complete.command, "_complete");
    assert!(complete.summary.contains("rust"));
}
