use std::sync::OnceLock;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TreeSitterCorpusCase {
    pub name: &'static str,
    pub source: &'static str,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TreeSitterQuerySnapshot {
    pub name: &'static str,
    pub query: &'static str,
}

const GRAMMAR_SOURCE: &str = include_str!("../tree-sitter/grammar.js");
const TREE_SITTER_CONFIG: &str = include_str!("../tree-sitter/tree-sitter.json");
const HIGHLIGHTS_QUERY_BASE: &str = include_str!("../queries/fol/highlights.base.scm");
#[cfg(test)]
const CHECKED_IN_HIGHLIGHTS_QUERY: &str = include_str!("../queries/fol/highlights.scm");
const LOCALS_QUERY: &str = include_str!("../queries/fol/locals.scm");
const SYMBOLS_QUERY: &str = include_str!("../queries/fol/symbols.scm");
const CORPUS_DECLARATIONS: &str = include_str!("../tree-sitter/test/corpus/declarations.txt");
const CORPUS_EXPRESSIONS: &str = include_str!("../tree-sitter/test/corpus/expressions.txt");
const CORPUS_RECOVERABLE: &str = include_str!("../tree-sitter/test/corpus/recoverable.txt");
const CORPUS_SHOWCASE: &str =
    include_str!("../../../../test/apps/showcases/full_v1_showcase/app/main.fol");
static GENERATED_HIGHLIGHTS_QUERY: OnceLock<String> = OnceLock::new();
static QUERY_SNAPSHOTS: OnceLock<[TreeSitterQuerySnapshot; 3]> = OnceLock::new();

pub fn fol_tree_sitter_grammar() -> &'static str {
    GRAMMAR_SOURCE
}

pub fn fol_tree_sitter_config() -> &'static str {
    TREE_SITTER_CONFIG
}

pub fn fol_tree_sitter_highlights_query() -> &'static str {
    GENERATED_HIGHLIGHTS_QUERY
        .get_or_init(generate_highlights_query)
        .as_str()
}

pub fn fol_tree_sitter_locals_query() -> &'static str {
    LOCALS_QUERY
}

pub fn fol_tree_sitter_symbols_query() -> &'static str {
    SYMBOLS_QUERY
}

pub fn fol_tree_sitter_corpus() -> &'static [TreeSitterCorpusCase] {
    &[
        TreeSitterCorpusCase {
            name: "declarations",
            source: CORPUS_DECLARATIONS,
        },
        TreeSitterCorpusCase {
            name: "expressions",
            source: CORPUS_EXPRESSIONS,
        },
        TreeSitterCorpusCase {
            name: "recoverable",
            source: CORPUS_RECOVERABLE,
        },
        TreeSitterCorpusCase {
            name: "showcase",
            source: CORPUS_SHOWCASE,
        },
    ]
}

pub fn fol_tree_sitter_query_snapshots() -> &'static [TreeSitterQuerySnapshot] {
    QUERY_SNAPSHOTS
        .get_or_init(|| {
            [
                TreeSitterQuerySnapshot {
                    name: "highlights",
                    query: fol_tree_sitter_highlights_query(),
                },
                TreeSitterQuerySnapshot {
                    name: "locals",
                    query: LOCALS_QUERY,
                },
                TreeSitterQuerySnapshot {
                    name: "symbols",
                    query: SYMBOLS_QUERY,
                },
            ]
        })
        .as_slice()
}

fn generate_highlights_query() -> String {
    HIGHLIGHTS_QUERY_BASE
        .replace("__FOL_SOURCE_KIND_LINES__", &render_source_kind_lines())
        .replace("__FOL_CONTAINER_TYPE_LINES__", &render_container_type_lines())
        .replace("__FOL_SHELL_TYPE_LINES__", &render_shell_type_lines())
        .replace(
            "__FOL_BUILTIN_TYPE_REGEX__",
            &render_group_regex(fol_typecheck::editor_builtin_type_names().iter().copied()),
        )
        .replace(
            "__FOL_DOT_INTRINSIC_REGEX__",
            &render_group_regex(
                fol_typecheck::editor_implemented_intrinsics()
                    .into_iter()
                    .filter(|entry| entry.surface == fol_intrinsics::IntrinsicSurface::DotRootCall)
                    .map(|entry| entry.name),
            ),
        )
}

fn render_group_regex<'a>(names: impl IntoIterator<Item = &'a str>) -> String {
    let joined = names.into_iter().collect::<Vec<_>>().join("|");
    format!("^({joined})$")
}

fn render_source_kind_lines() -> String {
    fol_typecheck::editor_source_kind_names()
        .iter()
        .map(|name| format!("(use_decl source_kind: (source_kind \"{name}\" @keyword.import))"))
        .collect::<Vec<_>>()
        .join("\n")
}

fn render_container_type_lines() -> String {
    fol_typecheck::editor_container_type_names()
        .iter()
        .map(|name| format!("(container_type \"{name}\" @type.builtin)"))
        .collect::<Vec<_>>()
        .join("\n")
}

fn render_shell_type_lines() -> String {
    fol_typecheck::editor_shell_type_names()
        .iter()
        .map(|name| format!("(shell_type \"{name}\" @type.builtin)"))
        .collect::<Vec<_>>()
        .join("\n")
}

#[cfg(test)]
mod tests {
    use super::{
        fol_tree_sitter_config, fol_tree_sitter_corpus, fol_tree_sitter_grammar,
        fol_tree_sitter_highlights_query, fol_tree_sitter_locals_query,
        fol_tree_sitter_query_snapshots, fol_tree_sitter_symbols_query,
        CHECKED_IN_HIGHLIGHTS_QUERY, HIGHLIGHTS_QUERY_BASE,
    };
    use std::path::{Path, PathBuf};
    use std::process::Command;

    fn repo_root() -> PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../../..").canonicalize().expect("repo root should resolve")
    }

    fn temp_root(label: &str) -> PathBuf {
        std::env::temp_dir().join(format!(
            "fol_editor_tree_query_{}_{}_{}",
            label,
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .expect("system time should be after epoch")
                .as_nanos()
        ))
    }

    fn build_bundle_root(label: &str) -> PathBuf {
        let root = temp_root(label);
        crate::editor_tree_generate_bundle(&root).expect("tree bundle generation should succeed");
        root
    }

    fn tree_sitter_cache_root(label: &str) -> PathBuf {
        let root = temp_root(&format!("cache_{label}"));
        std::fs::create_dir_all(&root).expect("tree-sitter cache root should be created");
        root
    }

    fn run_tree_sitter_query(
        bundle_root: &Path,
        query_path: &Path,
        source_path: &Path,
    ) -> std::process::Output {
        let cache_root = tree_sitter_cache_root("query");
        Command::new("tree-sitter")
            .env("XDG_CACHE_HOME", &cache_root)
            .arg("query")
            .arg("--grammar-path")
            .arg(bundle_root)
            .arg(query_path)
            .arg(source_path)
            .output()
            .expect("tree-sitter query should run")
    }

    #[test]
    fn grammar_scaffold_has_the_fol_language_name() {
        let grammar = fol_tree_sitter_grammar();
        assert!(grammar.contains("name: 'fol'"));
        assert!(grammar.contains("source_file:"));
    }

    #[test]
    fn tree_sitter_config_declares_fol_scope_and_queries() {
        let config = fol_tree_sitter_config();
        assert!(config.contains("\"scope\": \"source.fol\""));
        assert!(config.contains("\"file-types\": [\"fol\"]"));
        assert!(config.contains("\"highlights\": \"queries/fol/highlights.scm\""));
        assert!(config.contains("\"locals\": \"queries/fol/locals.scm\""));
    }

    #[test]
    fn grammar_covers_lexical_tokens_declarations_and_control_flow() {
        let grammar = fol_tree_sitter_grammar();
        for needle in [
            "identifier",
            "integer_literal",
            "string_literal",
            "comment",
            "doc_comment",
            "use_decl",
            "var_decl",
            "con_decl",
            "lab_decl",
            "fun_decl",
            "pro_decl",
            "log_decl",
            "typ_decl",
            "ali_decl",
            "def_decl",
            "seg_decl",
            "imp_decl",
            "std_decl",
            "block",
            "when_expr",
            "loop_expr",
            "if_stmt",
            "select_stmt",
            "while_stmt",
            "for_stmt",
            "each_stmt",
            "return_stmt",
            "yield_stmt",
            "defer_stmt",
            "report_stmt",
            "panic_stmt",
            "assert_stmt",
            "unreachable_stmt",
            "break_stmt",
        ] {
            assert!(
                grammar.contains(needle),
                "missing grammar rule marker: {needle}"
            );
        }
    }

    #[test]
    fn grammar_covers_v1_surface_families_explicitly() {
        let grammar = fol_tree_sitter_grammar();
        for needle in [
            "source_kind",
            "'loc'",
            "'std'",
            "'pkg'",
            "decl_modifiers",
            "modifier_list",
            "typed_binding",
            "method_decl",
            "record_type",
            "entry_type",
            "qualified_path",
            "dot_intrinsic",
            "check_expr",
            "container_type",
            "shell_type",
            "nil_literal",
            "unwrap_expr",
        ] {
            assert!(
                grammar.contains(needle),
                "missing v1 grammar marker: {needle}"
            );
        }
    }

    #[test]
    fn grammar_mentions_editor_friendly_recovery_shapes() {
        let grammar = fol_tree_sitter_grammar();
        assert!(grammar.contains("conflicts: $ => ["));
        assert!(grammar.contains("extras: $ => ["));
        assert!(grammar.contains("optional($.error_type)"));
        assert!(grammar.contains("$.field_access"));
        assert!(grammar.contains("$.boolean_literal"));
    }

    #[test]
    fn grammar_and_highlights_cover_compiler_declaration_keywords() {
        let grammar = fol_tree_sitter_grammar();
        let query = fol_tree_sitter_highlights_query();
        for keyword in fol_typecheck::editor_declaration_keywords() {
            let decl_rule = format!("{keyword}_decl");
            let quoted = format!("\"{keyword}\"");
            assert!(
                grammar.contains(&decl_rule) || grammar.contains(&quoted),
                "grammar is missing declaration keyword coverage for '{keyword}'"
            );
            assert!(
                query.contains(&quoted),
                "highlight query is missing declaration keyword coverage for '{keyword}'"
            );
        }
    }

    #[test]
    fn highlight_query_covers_declarations_keywords_and_literals() {
        let query = fol_tree_sitter_highlights_query();
        for needle in [
            "@keyword.import",
            "@keyword.function",
            "@keyword.type",
            "@keyword.conditional",
            "@keyword.return",
            "@keyword.exception",
            "@type",
            "@function",
            "@function.method",
            "@variable",
            "@punctuation.delimiter",
            "@punctuation.bracket",
            "@operator",
            "@constant.builtin",
            "@boolean",
            "@string",
            "@number",
            "@comment",
        ] {
            assert!(
                query.contains(needle),
                "missing highlight capture: {needle}"
            );
        }
    }

    #[test]
    fn highlight_query_base_template_keeps_generation_placeholders() {
        for needle in [
            "__FOL_SOURCE_KIND_LINES__",
            "__FOL_CONTAINER_TYPE_LINES__",
            "__FOL_SHELL_TYPE_LINES__",
            "__FOL_BUILTIN_TYPE_REGEX__",
            "__FOL_DOT_INTRINSIC_REGEX__",
        ] {
            assert!(
                HIGHLIGHTS_QUERY_BASE.contains(needle),
                "base highlight template is missing placeholder: {needle}"
            );
        }
    }

    #[test]
    fn generated_highlight_query_resolves_all_placeholders() {
        let query = fol_tree_sitter_highlights_query();
        for needle in [
            "__FOL_SOURCE_KIND_LINES__",
            "__FOL_CONTAINER_TYPE_LINES__",
            "__FOL_SHELL_TYPE_LINES__",
            "__FOL_BUILTIN_TYPE_REGEX__",
            "__FOL_DOT_INTRINSIC_REGEX__",
        ] {
            assert!(
                !query.contains(needle),
                "generated highlight query still contains placeholder: {needle}"
            );
        }
    }

    #[test]
    fn checked_in_highlight_query_matches_generated_output() {
        assert_eq!(CHECKED_IN_HIGHLIGHTS_QUERY, fol_tree_sitter_highlights_query());
    }

    #[test]
    fn highlight_query_covers_intrinsics_and_qualified_paths() {
        let query = fol_tree_sitter_highlights_query();
        assert!(query.contains("(dot_intrinsic \".\" @operator)"));
        assert!(query.contains("(dot_intrinsic name: (identifier) @function.builtin"));
        assert!(query.contains("#match? @function.builtin \"^("));
        assert!(query.contains("(qualified_path"));
        assert!(query.contains("@namespace"));
    }

    #[test]
    fn highlight_query_mentions_every_implemented_dot_intrinsic_name() {
        let query = fol_tree_sitter_highlights_query();
        for intrinsic in fol_typecheck::editor_implemented_intrinsics()
            .into_iter()
            .filter(|entry| entry.surface == fol_intrinsics::IntrinsicSurface::DotRootCall)
        {
            assert!(
                query.contains(intrinsic.name),
                "highlight query is missing implemented dot intrinsic '{}'",
                intrinsic.name
            );
        }
    }

    #[test]
    fn grammar_and_query_cover_bracketed_declaration_modifiers() {
        let grammar = fol_tree_sitter_grammar();
        let query = fol_tree_sitter_highlights_query();

        assert!(grammar.contains("optional(field('modifiers', $.decl_modifiers))"));
        assert!(grammar.contains("seq('[', optional($.modifier_list), ']')"));
        assert!(query
            .contains("(decl_modifiers \"[\" @punctuation.bracket \"]\" @punctuation.bracket)"));
        assert!(query.contains("(decl_modifiers (modifier_list (identifier) @attribute))"));
    }

    #[test]
    fn highlight_query_captures_each_declaration_head_explicitly() {
        let query = fol_tree_sitter_highlights_query();
        for needle in [
            "(use_decl \"use\" @keyword.import)",
            "(var_decl \"var\" @keyword)",
            "(con_decl \"con\" @keyword)",
            "(lab_decl \"lab\" @keyword)",
            "(fun_decl \"fun\" @keyword.function)",
            "(pro_decl \"pro\" @keyword.function)",
            "(log_decl \"log\" @keyword.function)",
            "(typ_decl \"typ\" @keyword.type)",
            "(ali_decl \"ali\" @keyword.type)",
            "(def_decl \"def\" @keyword.type)",
            "(seg_decl \"seg\" @keyword.type)",
            "(imp_decl \"imp\" @keyword.type)",
            "(std_decl \"std\" @keyword.type)",
            "(if_stmt \"if\" @keyword.conditional)",
            "(else_clause \"else\" @keyword.conditional)",
            "(select_stmt \"select\" @keyword.conditional)",
            "(when_expr \"when\" @keyword.conditional)",
            "(loop_expr \"loop\" @keyword.repeat)",
            "(while_stmt \"while\" @keyword.repeat)",
            "(for_stmt \"for\" @keyword.repeat)",
            "(each_stmt \"each\" @keyword.repeat)",
            "(return_stmt \"return\" @keyword.return)",
            "(yield_stmt \"yield\" @keyword.return)",
            "(defer_stmt \"defer\" @keyword.repeat)",
            "(report_stmt \"report\" @keyword.exception)",
            "(panic_stmt \"panic\" @keyword.exception)",
            "(assert_stmt \"assert\" @keyword.exception)",
            "(unreachable_stmt) @keyword.exception",
            "(check_expr \"check\" @keyword.exception)",
            "(break_stmt \"break\" @keyword.repeat)",
        ] {
            assert!(
                query.contains(needle),
                "missing declaration head capture: {needle}"
            );
        }
    }

    #[test]
    fn highlight_query_distinguishes_declaration_names_by_role() {
        let query = fol_tree_sitter_highlights_query();
        for needle in [
            "(use_decl name: (identifier) @namespace)",
            "(typ_decl name: (identifier) @type.definition)",
            "(ali_decl name: (identifier) @type.definition)",
            "(fun_decl declaration: (plain_fun_decl name: (identifier) @function))",
            "(pro_decl declaration: (plain_pro_decl name: (identifier) @function))",
            "(fun_decl declaration: (method_decl name: (identifier) @function.method))",
            "(log_decl declaration: (plain_log_decl name: (identifier) @function))",
            "(log_decl declaration: (method_decl name: (identifier) @function.method))",
            "(typed_binding \":\" @punctuation.delimiter)",
            "(param \":\" @punctuation.delimiter)",
            "(return_type \":\" @punctuation.delimiter)",
            "(ali_decl \":\" @punctuation.delimiter)",
            "(typ_decl \":\" @punctuation.delimiter)",
            "(var_decl \"=\" @operator)",
            "(params \"(\" @punctuation.bracket \")\" @punctuation.bracket)",
            "(receiver \"(\" @punctuation.bracket \")\" @punctuation.bracket)",
            "(block \"{\" @punctuation.bracket \"}\" @punctuation.bracket)",
            "(type_block \"{\" @punctuation.bracket \"}\" @punctuation.bracket)",
            "(container_type \"[\" @punctuation.bracket \"]\" @punctuation.bracket)",
            "(shell_type \"[\" @punctuation.bracket \"]\" @punctuation.bracket)",
            "(container_type \"arr\" @type.builtin)",
            "(shell_type \"opt\" @type.builtin)",
            "(record_type) @type.builtin",
            "(entry_type) @type.builtin",
            "(typed_binding type: (type_expr (identifier) @type.builtin)",
            "(param type: (type_expr (identifier) @type.builtin)",
            "(return_type (type_expr (identifier) @type.builtin)",
            "(typed_binding type: (type_expr (identifier) @type)",
            "(param type: (type_expr (identifier) @type)",
            "(return_type (type_expr (identifier) @type)",
            "(typed_binding type: (type_expr (qualified_path) @type))",
            "(param type: (type_expr (qualified_path) @type))",
            "(return_type (type_expr (qualified_path) @type))",
            "(error_type \"/\" @operator)",
            "(type_block (typed_binding name: (identifier) @property))",
            "(var_decl (typed_binding name: (identifier) @constant)",
            "(var_decl (typed_binding name: (identifier) @variable)",
            "(con_decl (typed_binding name: (identifier) @constant))",
            "(lab_decl (typed_binding name: (identifier) @variable))",
            "(field_init name: (identifier) @property)",
            "(field_init \"=\" @operator)",
            "(field_access field: (identifier) @property)",
            "(dot_intrinsic \".\" @operator)",
            "(unwrap_expr \"!\" @operator)",
            "(nil_literal) @constant.builtin",
            "(boolean_literal) @boolean",
        ] {
            assert!(
                query.contains(needle),
                "missing declaration role capture: {needle}"
            );
        }
    }

    #[test]
    fn highlight_query_uses_current_declaration_field_shapes() {
        let grammar = fol_tree_sitter_grammar();
        let query = fol_tree_sitter_highlights_query();

        assert!(grammar.contains("field('declaration', choice($.plain_fun_decl, $.method_decl))"));
        assert!(grammar.contains("field('declaration', choice($.plain_log_decl, $.method_decl))"));
        assert!(grammar.contains(
            "seq('var', optional(field('modifiers', $.decl_modifiers)), $.typed_binding"
        ));

        for needle in [
            "(use_decl \"use\" @keyword.import)",
            "(fun_decl \"fun\" @keyword.function)",
            "(log_decl \"log\" @keyword.function)",
            "(typ_decl \"typ\" @keyword.type)",
            "(ali_decl \"ali\" @keyword.type)",
            "(use_decl name: (identifier) @namespace)",
            "(typ_decl name: (identifier) @type.definition)",
            "(ali_decl name: (identifier) @type.definition)",
            "(fun_decl declaration: (plain_fun_decl",
            "(fun_decl declaration: (method_decl",
            "(log_decl declaration: (plain_log_decl",
            "(log_decl declaration: (method_decl",
            "(params \"(\" @punctuation.bracket \")\" @punctuation.bracket)",
            "(block \"{\" @punctuation.bracket \"}\" @punctuation.bracket)",
            "(type_block \"{\" @punctuation.bracket \"}\" @punctuation.bracket)",
            "(field_access receiver:",
            "(field_access field: (identifier) @property)",
            "(qualified_path root: (identifier) @namespace)",
            "(qualified_path segment: (identifier) @namespace)",
            "(var_decl (typed_binding name: (identifier) @constant)",
            "(var_decl (typed_binding name: (identifier) @variable)",
        ] {
            assert!(
                query.contains(needle),
                "highlight query drifted away from the current grammar shape: {needle}"
            );
        }
    }

    #[test]
    fn locals_query_captures_bindings_parameters_and_function_names() {
        let query = fol_tree_sitter_locals_query();
        for needle in [
            "@local.scope",
            "@local.definition",
            "@local.reference",
            "(param name: (identifier) @local.definition)",
            "(var_decl (typed_binding name: (identifier) @local.definition))",
            "(fun_decl declaration: (plain_fun_decl name: (identifier) @local.definition.function))",
        ] {
            assert!(
                query.contains(needle),
                "missing locals capture marker: {needle}"
            );
        }
    }

    #[test]
    fn locals_query_covers_named_declaration_families_from_compiler_surface() {
        let query = fol_tree_sitter_locals_query();
        for needle in [
            "(fun_decl declaration: (plain_fun_decl name: (identifier) @local.definition.function))",
            "(fun_decl declaration: (method_decl name: (identifier) @local.definition.method))",
            "(pro_decl declaration: (plain_pro_decl name: (identifier) @local.definition.function))",
            "(pro_decl declaration: (method_decl name: (identifier) @local.definition.method))",
            "(log_decl declaration: (plain_log_decl name: (identifier) @local.definition.function))",
            "(log_decl declaration: (method_decl name: (identifier) @local.definition.method))",
            "(typ_decl name: (identifier) @local.definition.type)",
            "(ali_decl name: (identifier) @local.definition.type)",
            "(con_decl (typed_binding name: (identifier) @local.definition))",
            "(lab_decl (typed_binding name: (identifier) @local.definition))",
        ] {
            assert!(
                query.contains(needle),
                "locals query lost declaration-family capture: {needle}"
            );
        }
        for keyword in ["fun", "pro", "log", "typ", "ali", "var", "con", "lab"] {
            assert!(
                fol_typecheck::editor_declaration_keywords().contains(&keyword),
                "compiler declaration keyword surface drifted away from locals expectation for '{keyword}'"
            );
        }
    }

    #[test]
    fn symbols_query_captures_types_functions_bindings_and_namespaces() {
        let query = fol_tree_sitter_symbols_query();
        for needle in [
            "@symbol.scope",
            "@symbol.function",
            "@symbol.type",
            "@symbol.variable",
            "@symbol.namespace",
        ] {
            assert!(
                query.contains(needle),
                "missing symbol capture marker: {needle}"
            );
        }
    }

    #[test]
    fn symbols_query_covers_named_declaration_families_from_compiler_surface() {
        let query = fol_tree_sitter_symbols_query();
        for needle in [
            "(fun_decl declaration: (plain_fun_decl name: (identifier) @symbol.function))",
            "(fun_decl declaration: (method_decl name: (identifier) @symbol.method))",
            "(pro_decl declaration: (plain_pro_decl name: (identifier) @symbol.function))",
            "(pro_decl declaration: (method_decl name: (identifier) @symbol.method))",
            "(log_decl declaration: (plain_log_decl name: (identifier) @symbol.function))",
            "(log_decl declaration: (method_decl name: (identifier) @symbol.method))",
            "(typ_decl name: (identifier) @symbol.type)",
            "(ali_decl name: (identifier) @symbol.type)",
            "(var_decl (typed_binding name: (identifier) @symbol.variable))",
            "(con_decl (typed_binding name: (identifier) @symbol.variable))",
            "(lab_decl (typed_binding name: (identifier) @symbol.variable))",
            "(seg_decl name: (identifier) @symbol.namespace)",
            "(std_decl name: (identifier) @symbol.type)",
            "(use_decl name: (identifier) @symbol.namespace)",
        ] {
            assert!(
                query.contains(needle),
                "symbols query lost declaration-family capture: {needle}"
            );
        }
        for keyword in ["fun", "pro", "log", "typ", "ali", "var", "con", "lab", "seg", "std", "use"] {
            assert!(
                fol_typecheck::editor_declaration_keywords().contains(&keyword),
                "compiler declaration keyword surface drifted away from symbols expectation for '{keyword}'"
            );
        }
    }

    #[test]
    fn query_snapshots_stay_in_editor_consumable_order() {
        let snapshots = fol_tree_sitter_query_snapshots();
        assert_eq!(snapshots.len(), 3);
        assert_eq!(snapshots[0].name, "highlights");
        assert_eq!(snapshots[1].name, "locals");
        assert_eq!(snapshots[2].name, "symbols");
    }

    #[test]
    fn corpus_smoke_cases_cover_real_v1_surfaces() {
        let corpus = fol_tree_sitter_corpus();
        assert_eq!(corpus.len(), 4);
        assert!(corpus
            .iter()
            .any(|case| case.source.contains("use shared: loc")));
        assert!(corpus.iter().any(|case| case.source.contains("when(flag)")));
        assert!(corpus
            .iter()
            .any(|case| case.source.contains("report \"bad-input\"")));
        assert!(corpus
            .iter()
            .any(|case| case.source.contains("typ User: rec")));
        assert!(corpus
            .iter()
            .any(|case| case.source.contains("true") || case.source.contains("false")));
    }

    #[test]
    fn corpus_and_editor_fixtures_keep_quoted_import_targets_only() {
        let corpus = fol_tree_sitter_corpus();
        for case in corpus {
            if case.source.contains("use ") {
                assert!(
                    case.source.contains("{\""),
                    "tree-sitter corpus '{}' should keep quoted import targets:\n{}",
                    case.name,
                    case.source
                );
                assert!(
                    !case.source.contains("{std}")
                        && !case.source.contains("{json}")
                        && !case.source.contains("{../"),
                    "tree-sitter corpus '{}' should not keep stale unquoted import targets:\n{}",
                    case.name,
                    case.source
                );
            }
        }

        for relative in [
            "lang/tooling/fol-editor/tests/fixtures/formatter/imports.formatted.fol",
            "lang/tooling/fol-editor/tests/fixtures/formatter/imports.misformatted.fol",
        ] {
            let source = std::fs::read_to_string(repo_root().join(relative))
                .expect("editor fixture should read");
            assert!(
                source.contains("{\""),
                "editor fixture '{}' should use quoted import targets:\n{}",
                relative,
                source
            );
            assert!(
                !source.contains("{std}")
                    && !source.contains("{json}")
                    && !source.contains("{../"),
                "editor fixture '{}' should not use stale unquoted import targets:\n{}",
                relative,
                source
            );
        }
    }

    #[test]
    fn generated_bundle_highlight_query_validates_against_tree_sitter() {
        let root = build_bundle_root("valid");
        let output = run_tree_sitter_query(
            &root,
            &root.join("queries/fol/highlights.scm"),
            &repo_root().join("xtra/logtiny/src/log.fol"),
        );

        assert!(
            output.status.success(),
            "tree-sitter query failed:\nstdout:\n{}\nstderr:\n{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
        let stdout = String::from_utf8_lossy(&output.stdout);
        assert!(stdout.contains("function"));
        assert!(stdout.contains("attribute"));

        std::fs::remove_dir_all(root).ok();
    }

    #[test]
    fn generated_bundle_locals_query_captures_real_example_bindings_and_methods() {
        let root = build_bundle_root("locals_real_examples");
        let output = run_tree_sitter_query(
            &root,
            &root.join("queries/fol/locals.scm"),
            &repo_root().join("examples/core_records/src/main.fol"),
        );

        assert!(
            output.status.success(),
            "tree-sitter locals query failed:\nstdout:\n{}\nstderr:\n{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
        let stdout = String::from_utf8_lossy(&output.stdout);
        assert!(stdout.contains("@local.definition.type"));
        assert!(stdout.contains("@local.definition.method"));
        assert!(stdout.contains("@local.definition.function"));
        assert!(stdout.contains("@local.definition"));

        std::fs::remove_dir_all(root).ok();
    }

    #[test]
    fn generated_bundle_symbols_query_captures_real_example_symbols() {
        let root = build_bundle_root("symbols_real_examples");
        let output = run_tree_sitter_query(
            &root,
            &root.join("queries/fol/symbols.scm"),
            &repo_root().join("examples/core_records/src/main.fol"),
        );

        assert!(
            output.status.success(),
            "tree-sitter symbols query failed:\nstdout:\n{}\nstderr:\n{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
        let stdout = String::from_utf8_lossy(&output.stdout);
        assert!(stdout.contains("@symbol.type"));
        assert!(stdout.contains("@symbol.method"));
        assert!(stdout.contains("@symbol.function"));
        assert!(stdout.contains("@symbol.variable"));

        std::fs::remove_dir_all(root).ok();
    }

    #[test]
    fn generated_bundle_locals_query_captures_v2_generic_example_bindings() {
        let root = build_bundle_root("locals_v2_generic_example");
        let output = run_tree_sitter_query(
            &root,
            &root.join("queries/fol/locals.scm"),
            &repo_root().join("examples/generic_routine_pair_m1/src/main.fol"),
        );

        assert!(
            output.status.success(),
            "tree-sitter locals query failed for V2 generic example:\nstdout:\n{}\nstderr:\n{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
        let stdout = String::from_utf8_lossy(&output.stdout);
        for needle in [
            "local.definition.function",
            "local.definition",
            "pair",
            "left",
            "right",
            "value",
        ] {
            assert!(
                stdout.contains(needle),
                "V2 generic example locals query lost capture '{needle}':\n{stdout}"
            );
        }

        std::fs::remove_dir_all(root).ok();
    }

    #[test]
    fn generated_bundle_symbols_query_captures_v2_standards_example_symbols() {
        let root = build_bundle_root("symbols_v2_standards_example");
        let output = run_tree_sitter_query(
            &root,
            &root.join("queries/fol/symbols.scm"),
            &repo_root().join("examples/standards_protocol_pair_m2/src/main.fol"),
        );

        assert!(
            output.status.success(),
            "tree-sitter symbols query failed for V2 standards example:\nstdout:\n{}\nstderr:\n{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
        let stdout = String::from_utf8_lossy(&output.stdout);
        for needle in [
            "symbol.type",
            "symbol.function",
            "Rect",
            "area",
            "perimeter",
            "main",
        ] {
            assert!(
                stdout.contains(needle),
                "V2 standards example symbols query lost capture '{needle}':\n{stdout}"
            );
        }

        std::fs::remove_dir_all(root).ok();
    }

    #[test]
    fn generated_bundle_v2_malformed_syntax_keeps_locals_and_symbols_sane() {
        let root = build_bundle_root("malformed_v2_locals_symbols");
        let source = root.join("fixtures/malformed_v2.fol");
        std::fs::create_dir_all(source.parent().expect("fixture parent should exist")).unwrap();
        std::fs::write(
            &source,
            concat!(
                "std geo: pro = {\n",
                "    fun area(: int;\n",
                "};\n",
                "fun pick(T)(value: T): T = {\n",
                "    return value;\n",
                "};\n",
            ),
        )
        .unwrap();

        for query_name in ["locals", "symbols"] {
            let output = run_tree_sitter_query(
                &root,
                &root.join(format!("queries/fol/{query_name}.scm")),
                &source,
            );
            assert!(
                output.status.success(),
                "tree-sitter {query_name} query failed on malformed V2 syntax:\nstdout:\n{}\nstderr:\n{}",
                String::from_utf8_lossy(&output.stdout),
                String::from_utf8_lossy(&output.stderr)
            );
            let stdout = String::from_utf8_lossy(&output.stdout);
            assert!(
                !stdout.contains("Query error") && !stdout.contains("Invalid node type"),
                "tree-sitter {query_name} query should stay sane on malformed V2 syntax:\n{stdout}"
            );
        }

        std::fs::remove_dir_all(root).ok();
    }

    #[test]
    fn v2_tree_sitter_coverage_keeps_highlights_distinct_from_locals_and_symbols() {
        let highlights = fol_tree_sitter_highlights_query();
        let locals = fol_tree_sitter_locals_query();
        let symbols = fol_tree_sitter_symbols_query();

        assert!(
            highlights.contains("@keyword") || highlights.contains("@function"),
            "highlight query should keep visual capture coverage"
        );
        assert!(
            locals.contains("@local.definition"),
            "locals query should keep explicit local-definition coverage"
        );
        assert!(
            symbols.contains("@symbol.function"),
            "symbols query should keep explicit symbol coverage"
        );
        assert!(
            !highlights.contains("@local.definition") && !highlights.contains("@symbol.function"),
            "highlight-only audits must not be mistaken for locals/symbol coverage"
        );
    }

    #[test]
    fn generated_bundle_highlights_keep_build_file_model_declarations_queryable() {
        let root = build_bundle_root("model_build_file");
        let output = run_tree_sitter_query(
            &root,
            &root.join("queries/fol/highlights.scm"),
            &repo_root().join("examples/mixed_models_workspace/build.fol"),
        );

        assert!(
            output.status.success(),
            "tree-sitter highlight query failed for build model declarations:\nstdout:\n{}\nstderr:\n{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
        let stdout = String::from_utf8_lossy(&output.stdout);
        assert!(stdout.contains("core"));
        assert!(stdout.contains("memo"));
        assert!(stdout.contains("std"));
        assert!(stdout.contains("property"));

        std::fs::remove_dir_all(root).ok();
    }

    #[test]
    fn generated_bundle_highlights_keep_real_bundled_std_import_examples_queryable() {
        let root = build_bundle_root("std_import_examples");
        for relative in [
            "examples/std_bundled_fmt/src/main.fol",
            "examples/std_alias_pkg/src/main.fol",
        ] {
            let output = run_tree_sitter_query(
                &root,
                &root.join("queries/fol/highlights.scm"),
                &repo_root().join(relative),
            );

            assert!(
                output.status.success(),
                "tree-sitter highlight query failed for '{}':\nstdout:\n{}\nstderr:\n{}",
                relative,
                String::from_utf8_lossy(&output.stdout),
                String::from_utf8_lossy(&output.stderr)
            );
            let stdout = String::from_utf8_lossy(&output.stdout);
            assert!(stdout.contains("namespace"), "expected namespace captures for '{}':\n{}", relative, stdout);
            assert!(stdout.contains("function"), "expected function captures for '{}':\n{}", relative, stdout);
        }

        std::fs::remove_dir_all(root).ok();
    }

    #[test]
    fn invalid_highlight_query_node_references_fail_bundle_validation() {
        let root = build_bundle_root("invalid");
        let query_path = root.join("queries/fol/highlights.scm");
        std::fs::write(&query_path, "(missing_fol_node) @keyword").unwrap();

        let output = run_tree_sitter_query(
            &root,
            &query_path,
            &repo_root().join("xtra/logtiny/src/log.fol"),
        );

        assert!(
            !output.status.success(),
            "invalid query unexpectedly succeeded:\nstdout:\n{}\nstderr:\n{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
        assert!(
            String::from_utf8_lossy(&output.stderr).contains("Invalid node type")
                || String::from_utf8_lossy(&output.stderr).contains("Query error"),
            "unexpected error output:\nstdout:\n{}\nstderr:\n{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );

        std::fs::remove_dir_all(root).ok();
    }

    #[test]
    fn declaration_heavy_real_fixtures_keep_highlight_captures_stable() {
        let root = build_bundle_root("declaration_snapshots");
        let logtiny_output = run_tree_sitter_query(
            &root,
            &root.join("queries/fol/highlights.scm"),
            &repo_root().join("xtra/logtiny/src/log.fol"),
        );
        assert!(logtiny_output.status.success());
        let logtiny = String::from_utf8_lossy(&logtiny_output.stdout);
        for needle in [
            "keyword.type",
            "type.definition",
            "attribute",
            "function",
            "boolean",
            "property",
            "variable.parameter",
        ] {
            assert!(
                logtiny.contains(needle),
                "declaration-heavy package fixture lost highlight capture: {needle}\n{logtiny}"
            );
        }

        let showcase_output = run_tree_sitter_query(
            &root,
            &root.join("queries/fol/highlights.scm"),
            &repo_root().join("test/apps/showcases/full_v1_showcase/app/main.fol"),
        );
        assert!(showcase_output.status.success());
        let showcase = String::from_utf8_lossy(&showcase_output.stdout);
        for needle in [
            "keyword.function",
            "type",
            "function.builtin",
            "variable",
            "property",
        ] {
            assert!(
                showcase.contains(needle),
                "showcase fixture lost declaration highlight capture: {needle}\n{showcase}"
            );
        }

        std::fs::remove_dir_all(root).ok();
    }

    #[test]
    fn control_and_effect_keywords_stay_highlighted_in_real_fixtures() {
        let root = build_bundle_root("keyword_effects");
        let output = run_tree_sitter_query(
            &root,
            &root.join("queries/fol/highlights.scm"),
            &repo_root().join("test/apps/fixtures/intrinsics_panic_check/main.fol"),
        );
        assert!(output.status.success());
        let stdout = String::from_utf8_lossy(&output.stdout);
        for needle in [
            "keyword.conditional",
            "keyword.return",
            "keyword.exception",
        ] {
            assert!(
                stdout.contains(needle),
                "control/effect fixture lost keyword capture: {needle}\n{stdout}"
            );
        }

        std::fs::remove_dir_all(root).ok();
    }

    #[test]
    fn import_source_kinds_keep_distinct_keyword_captures() {
        let query = fol_tree_sitter_highlights_query();
        for needle in [
            "(use_decl source_kind: (source_kind \"loc\" @keyword.import))",
            "(use_decl source_kind: (source_kind \"pkg\" @keyword.import))",
            "(use_decl source_kind: (source_kind \"std\" @keyword.import))",
        ] {
            assert!(
                query.contains(needle),
                "missing source-kind capture: {needle}"
            );
        }

        let root = build_bundle_root("import_source_kinds");
        let output = run_tree_sitter_query(
            &root,
            &root.join("queries/fol/highlights.scm"),
            &repo_root().join("test/apps/fixtures/mixed_loc_std_pkg/app/main.fol"),
        );
        assert!(output.status.success());
        let stdout = String::from_utf8_lossy(&output.stdout);
        for needle in ["namespace", "string", "keyword.function"] {
            assert!(
                stdout.contains(needle),
                "mixed import fixture lost source-kind capture: {needle}\n{stdout}"
            );
        }

        std::fs::remove_dir_all(root).ok();
    }

    #[test]
    fn shell_surfaces_keep_nil_and_boundary_captures() {
        let root = build_bundle_root("shell_surfaces");
        let optional_output = run_tree_sitter_query(
            &root,
            &root.join("queries/fol/highlights.scm"),
            &repo_root().join("test/apps/fixtures/shell_optional/main.fol"),
        );
        assert!(optional_output.status.success());
        let optional = String::from_utf8_lossy(&optional_output.stdout);
        for needle in ["constant.builtin", "type.builtin"] {
            assert!(
                optional.contains(needle),
                "optional shell fixture lost shell capture: {needle}\n{optional}"
            );
        }

        let boundary_output = run_tree_sitter_query(
            &root,
            &root.join("queries/fol/highlights.scm"),
            &repo_root().join("test/apps/fixtures/shell_vs_recoverable_boundary/main.fol"),
        );
        assert!(boundary_output.status.success());
        let boundary = String::from_utf8_lossy(&boundary_output.stdout);
        for needle in ["constant.builtin", "operator"] {
            assert!(
                boundary.contains(needle),
                "recoverable boundary fixture lost shell capture: {needle}\n{boundary}"
            );
        }

        std::fs::remove_dir_all(root).ok();
    }

    #[test]
    fn keyword_and_import_heavy_real_fixtures_keep_snapshot_shape() {
        let root = build_bundle_root("keyword_import_snapshots");
        let mixed_output = run_tree_sitter_query(
            &root,
            &root.join("queries/fol/highlights.scm"),
            &repo_root().join("test/apps/fixtures/mixed_loc_std_pkg/app/main.fol"),
        );
        assert!(mixed_output.status.success());
        let mixed = String::from_utf8_lossy(&mixed_output.stdout);
        for needle in [
            "namespace",
            "keyword.function",
            "keyword.return",
        ] {
            assert!(
                mixed.contains(needle),
                "mixed import fixture lost keyword/import capture: {needle}\n{mixed}"
            );
        }

        let panic_output = run_tree_sitter_query(
            &root,
            &root.join("queries/fol/highlights.scm"),
            &repo_root().join("test/apps/fixtures/intrinsics_panic_check/main.fol"),
        );
        assert!(panic_output.status.success());
        let panic_fixture = String::from_utf8_lossy(&panic_output.stdout);
        for needle in [
            "keyword.exception",
            "keyword.conditional",
            "keyword.return",
            "operator",
        ] {
            assert!(
                panic_fixture.contains(needle),
                "panic/check fixture lost keyword snapshot capture: {needle}\n{panic_fixture}"
            );
        }

        std::fs::remove_dir_all(root).ok();
    }

    #[test]
    fn builtin_and_named_type_references_keep_distinct_highlight_captures() {
        let root = build_bundle_root("type_references");
        let logtiny_output = run_tree_sitter_query(
            &root,
            &root.join("queries/fol/highlights.scm"),
            &repo_root().join("xtra/logtiny/src/log.fol"),
        );
        assert!(logtiny_output.status.success());
        let logtiny = String::from_utf8_lossy(&logtiny_output.stdout);
        for needle in ["type.builtin", "type.definition", "type"] {
            assert!(
                logtiny.contains(needle),
                "logtiny fixture lost type capture: {needle}\n{logtiny}"
            );
        }

        let showcase_output = run_tree_sitter_query(
            &root,
            &root.join("queries/fol/highlights.scm"),
            &repo_root().join("test/apps/showcases/full_v1_showcase/shared/lib.fol"),
        );
        assert!(showcase_output.status.success());
        let showcase = String::from_utf8_lossy(&showcase_output.stdout);
        for needle in ["type.builtin", "type", "namespace"] {
            assert!(
                showcase.contains(needle),
                "showcase fixture lost named type reference capture: {needle}\n{showcase}"
            );
        }

        std::fs::remove_dir_all(root).ok();
    }

    #[test]
    fn typed_binding_and_annotation_surfaces_keep_punctuation_captures() {
        let root = build_bundle_root("type_punctuation");
        let showcase_output = run_tree_sitter_query(
            &root,
            &root.join("queries/fol/highlights.scm"),
            &repo_root().join("test/apps/showcases/full_v1_showcase/app/main.fol"),
        );
        assert!(showcase_output.status.success());
        let showcase = String::from_utf8_lossy(&showcase_output.stdout);
        for needle in [
            "punctuation.delimiter",
            "punctuation.bracket",
            "type.builtin",
        ] {
            assert!(
                showcase.contains(needle),
                "showcase fixture lost type annotation capture: {needle}\n{showcase}"
            );
        }

        let shell_output = run_tree_sitter_query(
            &root,
            &root.join("queries/fol/highlights.scm"),
            &repo_root().join("test/apps/fixtures/shell_optional/main.fol"),
        );
        assert!(shell_output.status.success());
        let shell = String::from_utf8_lossy(&shell_output.stdout);
        for needle in [
            "punctuation.delimiter",
            "punctuation.bracket",
            "type.builtin",
        ] {
            assert!(
                shell.contains(needle),
                "shell fixture lost type annotation capture: {needle}\n{shell}"
            );
        }

        std::fs::remove_dir_all(root).ok();
    }

    #[test]
    fn dotted_intrinsics_keep_family_highlight_captures() {
        let root = build_bundle_root("intrinsic_families");
        let comparison_output = run_tree_sitter_query(
            &root,
            &root.join("queries/fol/highlights.scm"),
            &repo_root().join("test/apps/fixtures/intrinsics_comparison/main.fol"),
        );
        assert!(comparison_output.status.success());
        let comparison = String::from_utf8_lossy(&comparison_output.stdout);
        for needle in ["function.builtin", "operator"] {
            assert!(
                comparison.contains(needle),
                "comparison intrinsic fixture lost intrinsic capture: {needle}\n{comparison}"
            );
        }

        let echo_output = run_tree_sitter_query(
            &root,
            &root.join("queries/fol/highlights.scm"),
            &repo_root().join("test/apps/fixtures/intrinsics_not_len_echo/main.fol"),
        );
        assert!(echo_output.status.success());
        let echo = String::from_utf8_lossy(&echo_output.stdout);
        for needle in ["function.builtin", "operator"] {
            assert!(
                echo.contains(needle),
                "len/echo fixture lost intrinsic capture: {needle}\n{echo}"
            );
        }

        std::fs::remove_dir_all(root).ok();
    }

    #[test]
    fn container_shell_and_intrinsic_fixtures_keep_snapshot_shape() {
        let root = build_bundle_root("container_shell_intrinsic_snapshots");
        let container_output = run_tree_sitter_query(
            &root,
            &root.join("queries/fol/highlights.scm"),
            &repo_root().join("test/apps/fixtures/container_map_set/main.fol"),
        );
        assert!(container_output.status.success());
        let container = String::from_utf8_lossy(&container_output.stdout);
        for needle in [
            "type.builtin",
            "punctuation.bracket",
            "punctuation.delimiter",
            "function.builtin",
        ] {
            assert!(
                container.contains(needle),
                "container fixture lost snapshot capture: {needle}\n{container}"
            );
        }

        let shell_output = run_tree_sitter_query(
            &root,
            &root.join("queries/fol/highlights.scm"),
            &repo_root().join("test/apps/fixtures/shell_optional/main.fol"),
        );
        assert!(shell_output.status.success());
        let shell = String::from_utf8_lossy(&shell_output.stdout);
        for needle in [
            "type.builtin",
            "constant.builtin",
            "punctuation.bracket",
        ] {
            assert!(
                shell.contains(needle),
                "shell fixture lost snapshot capture: {needle}\n{shell}"
            );
        }

        let showcase_output = run_tree_sitter_query(
            &root,
            &root.join("queries/fol/highlights.scm"),
            &repo_root().join("test/apps/showcases/full_v1_showcase/app/main.fol"),
        );
        assert!(showcase_output.status.success());
        let showcase = String::from_utf8_lossy(&showcase_output.stdout);
        for needle in ["type.builtin", "function.builtin", "operator", "type"] {
            assert!(
                showcase.contains(needle),
                "showcase fixture lost container/shell/intrinsic capture: {needle}\n{showcase}"
            );
        }

        std::fs::remove_dir_all(root).ok();
    }

    #[test]
    fn tree_sitter_bundle_exercises_v1_niceties_and_model_examples() {
        let root = build_bundle_root("v1_niceties_and_models");
        let cases = [
            (
                repo_root().join("test/apps/fixtures/defer_scope_exit/main.fol"),
                ["keyword.exception", "punctuation.bracket"].as_slice(),
            ),
            (
                repo_root().join("test/apps/fixtures/call_binding_stress/main.fol"),
                ["punctuation.delimiter", "function"].as_slice(),
            ),
            (
                repo_root().join("examples/core_run_min/src/main.fol"),
                ["function", "type.builtin"].as_slice(),
            ),
            (
                repo_root().join("examples/generic_routine_m1/src/main.fol"),
                ["function", "type.builtin"].as_slice(),
            ),
            (
                repo_root().join("examples/generic_routine_pair_m1/src/main.fol"),
                ["function", "type.builtin"].as_slice(),
            ),
            (
                repo_root().join("examples/memo_run_min/src/main.fol"),
                ["function", "type.builtin"].as_slice(),
            ),
            (
                repo_root().join("examples/fail_generic_type_m1/src/main.fol"),
                ["type", "function"].as_slice(),
            ),
            (
                repo_root().join("examples/fail_generic_misuse_m1/src/main.fol"),
                ["type", "function"].as_slice(),
            ),
            (
                repo_root().join("examples/core_defer/src/main.fol"),
                ["keyword.exception", "type.builtin"].as_slice(),
            ),
            (
                repo_root().join("examples/standards_protocol_m2/src/main.fol"),
                ["function", "type"].as_slice(),
            ),
            (
                repo_root().join("examples/standards_protocol_pair_m2/src/main.fol"),
                ["function", "type"].as_slice(),
            ),
            (
                repo_root().join("examples/fail_standard_blueprint_m2/src/main.fol"),
                ["type", "keyword.import"].as_slice(),
            ),
            (
                repo_root().join("examples/fail_standard_as_type_m2/src/main.fol"),
                ["function", "type"].as_slice(),
            ),
            (
                repo_root().join("examples/std_bundled_fmt/src/main.fol"),
                ["keyword.import", "function", "type"].as_slice(),
            ),
            (
                repo_root().join("examples/std_bundled_io/src/main.fol"),
                ["keyword.import", "function", "type"].as_slice(),
            ),
            (
                repo_root().join("examples/std_echo_min/src/main.fol"),
                ["function.builtin", "operator"].as_slice(),
            ),
            (
                repo_root().join("examples/std_substrate_echo/src/main.fol"),
                ["function.builtin", "operator"].as_slice(),
            ),
        ];

        for (path, needles) in cases {
            let output =
                run_tree_sitter_query(&root, &root.join("queries/fol/highlights.scm"), &path);
            assert!(
                output.status.success(),
                "tree-sitter query failed for '{}':\nstdout:\n{}\nstderr:\n{}",
                path.display(),
                String::from_utf8_lossy(&output.stdout),
                String::from_utf8_lossy(&output.stderr)
            );
            let stdout = String::from_utf8_lossy(&output.stdout);
            for needle in needles {
                assert!(
                    stdout.contains(needle),
                    "fixture '{}' lost capture '{}':\n{}",
                    path.display(),
                    needle,
                    stdout
                );
            }
        }

        std::fs::remove_dir_all(root).ok();
    }
}
