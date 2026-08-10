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
    let emit =
        internal_complete_matches(&["code".to_string(), "emit".to_string(), "r".to_string()]);
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

#[test]
fn tool_completion_prints_a_real_script_for_every_supported_shell() {
    for shell in ["bash", "zsh", "fish"] {
        let (output, result) = run_command_from_args(["fol", "tool", "completion", shell])
            .expect("completion command should run");
        let rendered = output
            .render_command_summary(&result)
            .expect("completion output should render");

        assert!(
            rendered.lines().count() > 5,
            "`tool completion {shell}` printed no script: {rendered:?}"
        );
        assert!(
            !rendered.contains("Done:"),
            "`tool completion {shell}` wrapped the script in a status envelope: {rendered}"
        );
        for subcommand in ["work", "pack", "code", "tool", "emit", "explain", "clean"] {
            assert!(
                rendered.contains(subcommand),
                "`tool completion {shell}` never mentions '{subcommand}': {rendered}"
            );
        }
    }
}

#[test]
fn tool_completion_json_envelope_carries_the_script() {
    let (output, result) =
        run_command_from_args(["fol", "--output", "json", "tool", "completion", "zsh"])
            .expect("completion command should run");
    let rendered = output
        .render_command_summary(&result)
        .expect("completion output should render");

    assert!(rendered.contains("\"payload\""), "rendered: {rendered}");
    assert!(rendered.contains("#compdef fol"), "rendered: {rendered}");
}
