use std::sync::OnceLock;
use tree_sitter::{Language, Parser, Query, QueryCursor, StreamingIterator, Tree};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TreeSitterCorpusCase {
    pub name: &'static str,
    pub source: &'static str,
}

impl TreeSitterCorpusCase {
    /// Return only the FOL program from this complete Tree-sitter corpus case.
    ///
    /// `source` intentionally stores the full `===` / `---` corpus fixture so
    /// bundle generation can preserve the checked-in expected syntax tree.
    pub fn program_source(&self) -> Option<&'static str> {
        let body = self
            .source
            .splitn(3, "==================")
            .nth(2)?
            .trim_start_matches(|character| character == '\r' || character == '\n');
        let (program, _) = body.split_once("\n---\n")?;
        Some(program.trim_end_matches(|character| character == '\r' || character == '\n'))
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TreeSitterQuerySnapshot {
    pub name: &'static str,
    pub query: &'static str,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct TreeSitterSyntaxIssue {
    pub kind: String,
    pub start_row: usize,
    pub start_column: usize,
    pub end_row: usize,
    pub end_column: usize,
    pub text: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct TreeSitterParseResult {
    pub root_kind: String,
    pub syntax_tree: String,
    pub node_count: usize,
    pub named_node_count: usize,
    pub errors: Vec<TreeSitterSyntaxIssue>,
    pub missing: Vec<TreeSitterSyntaxIssue>,
}

impl TreeSitterParseResult {
    pub fn has_error(&self) -> bool {
        !self.errors.is_empty() || !self.missing.is_empty()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct TreeSitterQueryCapture {
    pub name: String,
    pub start_row: usize,
    pub start_column: usize,
    pub end_row: usize,
    pub end_column: usize,
    pub text: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct TreeSitterQueryResult {
    pub parse: TreeSitterParseResult,
    pub captures: Vec<TreeSitterQueryCapture>,
}

unsafe extern "C" {
    fn tree_sitter_fol() -> *const tree_sitter::ffi::TSLanguage;
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
const CORPUS_V3_OWNERSHIP: &str = include_str!("../tree-sitter/test/corpus/v3_ownership.txt");
const CORPUS_V3_POINTERS: &str = include_str!("../tree-sitter/test/corpus/v3_pointers.txt");
const CORPUS_V3_CHANNELS_SELECT_MUTEX: &str =
    include_str!("../tree-sitter/test/corpus/v3_channels_select_mutex.txt");
const CORPUS_V3_EVENTUALS: &str = include_str!("../tree-sitter/test/corpus/v3_eventuals.txt");
const CORPUS_V3_DEFERRED: &str = include_str!("../tree-sitter/test/corpus/v3_deferred.txt");
const CORPUS_V3_LEXICAL_BOUNDARIES: &str =
    include_str!("../tree-sitter/test/corpus/v3_lexical_boundaries.txt");
const SHOWCASE_FIXTURE: &str =
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

pub(crate) fn fol_tree_sitter_showcase_fixture() -> &'static str {
    SHOWCASE_FIXTURE
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
            name: "v3_ownership",
            source: CORPUS_V3_OWNERSHIP,
        },
        TreeSitterCorpusCase {
            name: "v3_pointers",
            source: CORPUS_V3_POINTERS,
        },
        TreeSitterCorpusCase {
            name: "v3_channels_select_mutex",
            source: CORPUS_V3_CHANNELS_SELECT_MUTEX,
        },
        TreeSitterCorpusCase {
            name: "v3_eventuals",
            source: CORPUS_V3_EVENTUALS,
        },
        TreeSitterCorpusCase {
            name: "v3_deferred",
            source: CORPUS_V3_DEFERRED,
        },
        TreeSitterCorpusCase {
            name: "v3_lexical_boundaries",
            source: CORPUS_V3_LEXICAL_BOUNDARIES,
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
        .replace(
            "__FOL_CONTAINER_TYPE_LINES__",
            &render_container_type_lines(),
        )
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

pub(crate) fn execute_fol_tree_sitter_parse(source: &str) -> Result<TreeSitterParseResult, String> {
    let language = fol_tree_sitter_language()?;
    let tree = parse_fol_source(source, &language)?;
    Ok(inspect_tree(&tree, source))
}

pub(crate) fn execute_fol_tree_sitter_query(
    source: &str,
    query_source: &str,
) -> Result<TreeSitterQueryResult, String> {
    let language = fol_tree_sitter_language()?;
    let tree = parse_fol_source(source, &language)?;
    let parse = inspect_tree(&tree, source);
    let query = Query::new(&language, query_source)
        .map_err(|error| format!("failed to compile FOL tree-sitter query: {error}"))?;
    let capture_names = query.capture_names();
    let mut cursor = QueryCursor::new();
    let mut query_captures = cursor.captures(&query, tree.root_node(), source.as_bytes());
    let mut captures = Vec::new();

    while let Some((query_match, capture_index)) = query_captures.next() {
        let capture = query_match.captures[*capture_index];
        let node = capture.node;
        let start = node.start_position();
        let end = node.end_position();
        captures.push(TreeSitterQueryCapture {
            name: capture_names[capture.index as usize].to_string(),
            start_row: start.row,
            start_column: start.column,
            end_row: end.row,
            end_column: end.column,
            text: node
                .utf8_text(source.as_bytes())
                .unwrap_or_default()
                .to_string(),
        });
    }

    Ok(TreeSitterQueryResult { parse, captures })
}

fn fol_tree_sitter_language() -> Result<Language, String> {
    let pointer = unsafe { tree_sitter_fol() };
    if pointer.is_null() {
        return Err("generated FOL tree-sitter language returned a null pointer".to_string());
    }
    Ok(unsafe { Language::from_raw(pointer) })
}

fn parse_fol_source(source: &str, language: &Language) -> Result<Tree, String> {
    let mut parser = Parser::new();
    parser
        .set_language(language)
        .map_err(|error| format!("failed to load generated FOL tree-sitter parser: {error}"))?;
    parser
        .parse(source, None)
        .ok_or_else(|| "FOL tree-sitter parse was cancelled".to_string())
}

fn inspect_tree(tree: &Tree, source: &str) -> TreeSitterParseResult {
    let root = tree.root_node();
    let mut stack = vec![root];
    let mut node_count = 0usize;
    let mut named_node_count = 0usize;
    let mut errors = Vec::new();
    let mut missing = Vec::new();

    while let Some(node) = stack.pop() {
        node_count += 1;
        named_node_count += usize::from(node.is_named());
        if node.is_error() {
            errors.push(syntax_issue(node, source, "ERROR"));
        }
        if node.is_missing() {
            missing.push(syntax_issue(
                node,
                source,
                &format!("MISSING {}", node.kind()),
            ));
        }

        let mut cursor = node.walk();
        let children = node.children(&mut cursor).collect::<Vec<_>>();
        stack.extend(children.into_iter().rev());
    }

    TreeSitterParseResult {
        root_kind: root.kind().to_string(),
        syntax_tree: root.to_sexp(),
        node_count,
        named_node_count,
        errors,
        missing,
    }
}

fn syntax_issue(node: tree_sitter::Node<'_>, source: &str, kind: &str) -> TreeSitterSyntaxIssue {
    let start = node.start_position();
    let end = node.end_position();
    TreeSitterSyntaxIssue {
        kind: kind.to_string(),
        start_row: start.row,
        start_column: start.column,
        end_row: end.row,
        end_column: end.column,
        text: node
            .utf8_text(source.as_bytes())
            .unwrap_or_default()
            .to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::{
        execute_fol_tree_sitter_parse, execute_fol_tree_sitter_query, fol_tree_sitter_config,
        fol_tree_sitter_corpus, fol_tree_sitter_grammar, fol_tree_sitter_highlights_query,
        fol_tree_sitter_locals_query, fol_tree_sitter_query_snapshots,
        fol_tree_sitter_symbols_query, CHECKED_IN_HIGHLIGHTS_QUERY, HIGHLIGHTS_QUERY_BASE,
    };
    use fol_lexer::token::buildin::{
        CONTROL_KEYWORDS, DECLARATION_KEYWORDS, DIAGNOSTIC_KEYWORDS, LITERAL_KEYWORDS,
        OPERATOR_KEYWORDS, OTHER_KEYWORDS,
    };
    use std::collections::BTreeSet;
    use std::path::{Path, PathBuf};
    use std::process::Command;

    fn repo_root() -> PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../..")
            .canonicalize()
            .expect("repo root should resolve")
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

    fn assert_quoted_import_targets(label: &str, source: &str) {
        let result = execute_fol_tree_sitter_query(
            source,
            "(use_decl target: (_) @import_target)",
        )
        .unwrap_or_else(|error| panic!("failed to inspect import targets in '{label}': {error}"));
        assert!(
            !result.parse.has_error(),
            "'{label}' should parse before its import targets are audited:\nerrors={:?}\nmissing={:?}\n{source}",
            result.parse.errors,
            result.parse.missing,
        );
        for capture in result
            .captures
            .iter()
            .filter(|capture| capture.name == "import_target")
        {
            let target = capture.text.trim();
            assert!(
                target.len() >= 2 && target.starts_with('"') && target.ends_with('"'),
                "'{label}' should keep quoted import targets, got '{target}':\n{source}"
            );
        }
    }

    fn run_tree_sitter_query(
        bundle_root: &Path,
        query_path: &Path,
        source_path: &Path,
    ) -> std::process::Output {
        let cache_root = tree_sitter_cache_root("query");
        let output = Command::new("tree-sitter")
            .env("XDG_CACHE_HOME", &cache_root)
            .arg("query")
            .arg("--grammar-path")
            .arg(bundle_root)
            .arg(query_path)
            .arg(source_path)
            .output()
            .expect("tree-sitter query should run");
        std::fs::remove_dir_all(cache_root).ok();
        output
    }

    fn run_tree_sitter_parse(
        bundle_root: &Path,
        cache_root: &Path,
        source_path: &Path,
    ) -> std::process::Output {
        Command::new("tree-sitter")
            .env("XDG_CACHE_HOME", cache_root)
            .arg("parse")
            .arg("--grammar-path")
            .arg(bundle_root)
            .arg(source_path)
            .output()
            .expect("tree-sitter parse should run")
    }

    fn run_tree_sitter_parse_many(
        bundle_root: &Path,
        cache_root: &Path,
        source_paths: &[PathBuf],
    ) -> std::process::Output {
        Command::new("tree-sitter")
            .env("XDG_CACHE_HOME", cache_root)
            .arg("parse")
            .arg("--quiet")
            .arg("--grammar-path")
            .arg(bundle_root)
            .args(source_paths)
            .output()
            .expect("tree-sitter multi-file parse should run")
    }

    fn run_tree_sitter_test(bundle_root: &Path, cache_root: &Path) -> std::process::Output {
        Command::new("tree-sitter")
            .env("XDG_CACHE_HOME", cache_root)
            .arg("test")
            .current_dir(bundle_root)
            .output()
            .expect("tree-sitter corpus test should run")
    }

    fn collect_fol_sources(root: &Path, sources: &mut BTreeSet<PathBuf>) {
        let mut entries = std::fs::read_dir(root)
            .unwrap_or_else(|error| panic!("failed to read '{}': {error}", root.display()))
            .collect::<Result<Vec<_>, _>>()
            .unwrap_or_else(|error| panic!("failed to enumerate '{}': {error}", root.display()));
        entries.sort_by_key(|entry| entry.path());

        for entry in entries {
            let path = entry.path();
            let file_type = entry
                .file_type()
                .unwrap_or_else(|error| panic!("failed to inspect '{}': {error}", path.display()));
            if file_type.is_dir() {
                if entry.file_name() != ".fol" {
                    collect_fol_sources(&path, sources);
                }
            } else if file_type.is_file()
                && path.extension().and_then(|extension| extension.to_str()) == Some("fol")
            {
                sources.insert(path);
            }
        }
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
            "std_decl",
            "standard_block",
            "standard_requirement",
            "standard_field_requirement",
            "block",
            "generic_params",
            "generic_type_expr",
            "type_contract_claims",
            "turbofish_type_args",
            "when_expr",
            "loop_expr",
            "if_stmt",
            "select_stmt",
            "while_stmt",
            "for_stmt",
            "each_stmt",
            "return_stmt",
            "yield_stmt",
            "dfr_stmt",
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
            "unary_expr",
            "if_expr",
            "select_stmt",
            "range_expr",
            "anonymous_fun_expr",
            "anonymous_pro_expr",
            "anonymous_log_expr",
            "routine_capture_list",
            "container_type",
            "shell_type",
            "nil_literal",
            "unwrap_expr",
            "this_expr",
            "self_expr",
            "where_expr",
            "get_expr",
            "async_expr",
            "await_expr",
            "do_expr",
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
        assert_eq!(
            CHECKED_IN_HIGHLIGHTS_QUERY,
            fol_tree_sitter_highlights_query()
        );
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
    fn grammar_and_highlights_cover_compiler_operator_keywords() {
        let grammar = fol_tree_sitter_grammar();
        let query = fol_tree_sitter_highlights_query();

        for keyword in OPERATOR_KEYWORDS {
            let quoted = format!("\"{keyword}\"");
            assert!(
                grammar.contains(&format!("'{keyword}'")) || grammar.contains(&quoted),
                "grammar is missing operator keyword coverage for '{keyword}'"
            );
            assert!(
                query.contains(&format!("operator: {quoted} @operator")),
                "highlight query is missing operator keyword coverage for '{keyword}'"
            );
        }
    }

    #[test]
    fn grammar_references_every_compiler_keyword_surface() {
        let grammar = fol_tree_sitter_grammar();
        let all_keywords: BTreeSet<_> = DECLARATION_KEYWORDS
            .iter()
            .chain(CONTROL_KEYWORDS.iter())
            .chain(OPERATOR_KEYWORDS.iter())
            .chain(LITERAL_KEYWORDS.iter())
            .chain(DIAGNOSTIC_KEYWORDS.iter())
            .chain(OTHER_KEYWORDS.iter())
            .copied()
            .collect();

        assert_eq!(
            all_keywords.len(),
            53,
            "compiler keyword inventory changed; update editor summary coverage"
        );

        let missing = all_keywords
            .iter()
            .filter(|keyword| {
                let single = format!("'{keyword}'");
                let double = format!("\"{keyword}\"");
                !grammar.contains(&single) && !grammar.contains(&double)
            })
            .copied()
            .collect::<Vec<_>>();

        assert!(
            missing.is_empty(),
            "grammar is missing compiler keywords: {missing:?}"
        );
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
            "(var_decl \"@var\" @keyword)",
            "(var_decl \"~var\" @keyword)",
            "(con_decl \"con\" @keyword)",
            "(lab_decl \"lab\" @keyword)",
            "(fun_decl \"fun\" @keyword.function)",
            "(pro_decl \"pro\" @keyword.function)",
            "(log_decl \"log\" @keyword.function)",
            "(typ_decl \"typ\" @keyword.type)",
            "(ali_decl \"ali\" @keyword.type)",
            "(def_decl \"def\" @keyword.type)",
            "(seg_decl \"seg\" @keyword.type)",
            "(std_decl \"std\" @keyword.type)",
            "(standard_field_requirement \"var\" @keyword)",
            "(turbofish_type_args \"::[\" @punctuation.bracket \"]\" @punctuation.bracket)",
            "(if_stmt \"if\" @keyword.conditional)",
            "(if_expr \"if\" @keyword.conditional)",
            "(if_expr \"else\" @keyword.conditional)",
            "(select_stmt \"select\" @keyword.conditional)",
            "(when_expr \"when\" @keyword.conditional)",
            "(loop_expr \"loop\" @keyword.repeat)",
            "(while_stmt \"while\" @keyword.repeat)",
            "(for_stmt \"for\" @keyword.repeat)",
            "(each_stmt \"each\" @keyword.repeat)",
            "(return_stmt \"return\" @keyword.return)",
            "(yield_stmt \"yield\" @keyword.return)",
            "(dfr_stmt \"dfr\" @keyword.repeat)",
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
            "(anonymous_fun_expr \"fun\" @keyword.function)",
            "(anonymous_pro_expr \"pro\" @keyword.function)",
            "(anonymous_log_expr \"log\" @keyword.function)",
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
            "(generic_type_expr base: (identifier) @type.builtin",
            "(generic_type_expr base: (identifier) @type",
            "(generic_type_expr base: (qualified_path) @type)",
            "(type_expr (identifier) @type.builtin",
            "(type_expr (identifier) @type",
            "(type_expr (qualified_path) @type)",
            "(error_type \"/\" @operator)",
            "(record_field (typed_binding name: (identifier) @property))",
            "(var_decl (typed_binding name: (identifier) @constant)",
            "(var_decl (typed_binding name: (identifier) @variable)",
            "(con_decl (typed_binding name: (identifier) @constant))",
            "(lab_decl (typed_binding name: (identifier) @variable))",
            "(field_init name: (identifier) @property)",
            "(field_init \"=\" @operator)",
            "(field_access field: (identifier) @property)",
            "(binary_expr operator: \"^\" @operator)",
            "(range_expr \"..\" @operator)",
            "(range_expr \"...\" @operator)",
            "(dot_intrinsic \".\" @operator)",
            "(routine_capture_list \"[\" @punctuation.bracket \"]\" @punctuation.bracket)",
            "(routine_capture_list \",\" @punctuation.delimiter)",
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
        for keyword in [
            "fun", "pro", "log", "typ", "ali", "var", "con", "lab", "seg", "std", "use",
        ] {
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
    fn native_parser_and_highlights_match_compiler_lexical_boundaries() {
        let source = concat!(
            "/* block comment with [>] { chn[int] }\n",
            "   and a second line */\n",
            "` ordinary backtick comment with dfr {\n",
            "  edf { counter[mux] } } `\n",
            "`[doc] documentation with #view\n",
            "and !view on another line`\n",
            "`[trace] not a documentation comment`\n",
            "fun[] lexical(): str = {\n",
            "    var raw: str = 'raw [>] text\n",
            "with braces { }';\n",
            "    var empty: str = '';\n",
            "    var character: chr = 'z';\n",
            "    return raw;\n",
            "};\n",
        );

        let parse =
            execute_fol_tree_sitter_parse(source).expect("native generated parser should load");
        assert!(
            !parse.has_error(),
            "compiler-valid comments/raw quotes should parse without recovery: {parse:#?}"
        );
        for node in [
            "(comment)",
            "(doc_comment)",
            "(raw_string_literal)",
            "(char_literal)",
        ] {
            assert!(
                parse.syntax_tree.contains(node),
                "native syntax tree lost lexical node '{node}': {}",
                parse.syntax_tree
            );
        }

        let highlighted = execute_fol_tree_sitter_query(source, fol_tree_sitter_highlights_query())
            .expect("checked-in highlight query should execute");
        assert!(!highlighted.parse.has_error());
        for (capture, text) in [
            ("comment", "/* block comment"),
            ("comment", "` ordinary backtick"),
            ("comment.documentation", "`[doc] documentation"),
            ("comment", "`[trace] not a documentation"),
            ("string", "'raw [>] text"),
            ("string", "''"),
            ("character", "'z'"),
        ] {
            assert!(
                highlighted.captures.iter().any(|candidate| {
                    candidate.name == capture && candidate.text.starts_with(text)
                }),
                "highlight query lost @{capture} for '{text}': {:#?}",
                highlighted.captures
            );
        }
        assert!(
            highlighted.captures.iter().all(|candidate| {
                candidate.name != "comment.documentation" || !candidate.text.starts_with("`[trace]")
            }),
            "only the compiler's exact `[doc]` prefix may receive documentation highlighting"
        );
    }

    #[test]
    fn native_parser_rejects_the_removed_std_import_source_kind() {
        for source in [
            "use shared: loc = {\"../shared\"};\n",
            "use std: pkg = {\"std\"};\n",
        ] {
            let parse =
                execute_fol_tree_sitter_parse(source).expect("native generated parser should load");
            assert!(
                !parse.has_error(),
                "current public source kind should parse: {source}\n{parse:#?}"
            );
        }

        let removed = execute_fol_tree_sitter_parse("use std: std = {\"std\"};\n")
            .expect("native generated parser should load");
        assert!(
            removed.has_error(),
            "removed public std source kind must produce an ERROR node: {removed:#?}"
        );
    }

    #[test]
    fn corpus_smoke_cases_cover_real_language_surfaces() {
        let corpus = fol_tree_sitter_corpus();
        assert_eq!(corpus.len(), 9);
        for case in corpus {
            assert!(
                case.source.contains("\n---\n"),
                "tree-sitter corpus '{}' must include an expected syntax tree",
                case.name
            );
            assert!(
                case.program_source()
                    .is_some_and(|source| !source.is_empty()),
                "tree-sitter corpus '{}' must expose a non-empty FOL program",
                case.name
            );
        }
        assert!(corpus
            .iter()
            .any(|case| case.source.contains("use shared: loc")));
        assert!(corpus.iter().any(|case| case.source.contains("when(flag)")));
        assert!(corpus
            .iter()
            .any(|case| case.source.contains("[>]worker()")));
        assert!(corpus
            .iter()
            .any(|case| case.source.contains("report \"bad-input\"")));
        assert!(corpus
            .iter()
            .any(|case| case.source.contains("typ User: rec")));
        assert!(corpus
            .iter()
            .any(|case| case.source.contains("ali IntBox: Box[int]")));
        assert!(corpus
            .iter()
            .any(|case| case.name == "v3_ownership" && case.source.contains("~var replacement")));
        assert!(corpus.iter().any(|case| {
            case.name == "v3_pointers"
                && case.source.contains("ptr[Box[int]]")
                && case.source.contains("@arr[models::Node, 1]")
        }));
        assert!(corpus.iter().any(|case| {
            case.name == "v3_channels_select_mutex"
                && case.source.contains("select {")
                && case.source.contains("counter[mux]")
        }));
        assert!(corpus.iter().any(|case| {
            case.name == "v3_eventuals"
                && case.source.contains("[>]work(1)")
                && case.source.contains("| async")
                && case.source.contains("| await")
        }));
        assert!(corpus.iter().any(|case| {
            case.name == "v3_deferred"
                && case.source.contains("dfr {")
                && case.source.contains("edf {")
        }));
        assert!(corpus.iter().any(|case| {
            case.name == "v3_lexical_boundaries"
                && case
                    .source
                    .contains("/* A slash block comment may span lines")
                && case
                    .source
                    .contains("` A backtick comment may also span lines")
                && case.source.contains("var raw: str = 'raw text may span")
        }));
        assert!(corpus
            .iter()
            .any(|case| case.source.contains("true") || case.source.contains("false")));
    }

    #[test]
    fn dedicated_v3_corpus_sources_parse_without_error_nodes() {
        let root = build_bundle_root("dedicated_v3_corpus");
        let cache = tree_sitter_cache_root("dedicated_v3_corpus");

        for name in [
            "v3_ownership",
            "v3_pointers",
            "v3_channels_select_mutex",
            "v3_eventuals",
            "v3_deferred",
            "v3_lexical_boundaries",
        ] {
            let case = fol_tree_sitter_corpus()
                .iter()
                .find(|case| case.name == name)
                .unwrap_or_else(|| panic!("missing dedicated V3 corpus '{name}'"));
            let exported =
                std::fs::read_to_string(root.join("test/corpus").join(format!("{name}.txt")))
                    .unwrap_or_else(|error| {
                        panic!("failed to read exported V3 corpus '{name}': {error}")
                    });
            assert_eq!(
                exported, case.source,
                "generated tree-sitter bundle should export V3 corpus '{name}' exactly"
            );
            let source = case
                .program_source()
                .unwrap_or_else(|| panic!("V3 corpus '{name}' should be a complete corpus case"));
            let path = root.join(format!("{name}.fol"));
            std::fs::write(&path, source)
                .unwrap_or_else(|error| panic!("failed to write V3 corpus '{name}': {error}"));

            let output = run_tree_sitter_parse(&root, &cache, &path);
            let stdout = String::from_utf8_lossy(&output.stdout);
            assert!(
                output.status.success() && !stdout.contains("(ERROR"),
                "dedicated V3 corpus '{name}' should parse without ERROR nodes:\nstdout:\n{}\nstderr:\n{}",
                stdout,
                String::from_utf8_lossy(&output.stderr)
            );
        }

        std::fs::remove_dir_all(root).ok();
        std::fs::remove_dir_all(cache).ok();
    }

    #[test]
    fn generated_bundle_executes_real_tree_sitter_corpus_cases() {
        let root = build_bundle_root("external_corpus");
        let cache = tree_sitter_cache_root("external_corpus");
        let output = run_tree_sitter_test(&root, &cache);
        let stdout = String::from_utf8_lossy(&output.stdout);
        let stderr = String::from_utf8_lossy(&output.stderr);
        let case_count = fol_tree_sitter_corpus().len();

        assert!(
            output.status.success(),
            "external tree-sitter corpus test failed:\nstdout:\n{stdout}\nstderr:\n{stderr}"
        );
        assert!(
            stdout.contains(&format!(
                "Total parses: {case_count}; successful parses: {case_count}; failed parses: 0;"
            )),
            "external tree-sitter corpus test did not execute every registered case:\n{stdout}"
        );

        std::fs::remove_dir_all(root).ok();
        std::fs::remove_dir_all(cache).ok();
    }

    #[test]
    fn corpus_and_editor_fixtures_keep_quoted_import_targets_only() {
        let corpus = fol_tree_sitter_corpus();
        for case in corpus {
            let source = case
                .program_source()
                .unwrap_or_else(|| panic!("corpus '{}' should retain a program", case.name));
            assert_quoted_import_targets(&format!("tree-sitter corpus '{}'", case.name), source);
        }

        for relative in [
            "lang/tooling/fol-editor/tests/fixtures/formatter/imports.formatted.fol",
            "lang/tooling/fol-editor/tests/fixtures/formatter/imports.misformatted.fol",
        ] {
            let source = std::fs::read_to_string(repo_root().join(relative))
                .expect("editor fixture should read");
            assert_quoted_import_targets(&format!("editor fixture '{relative}'"), &source);
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
    fn generated_bundle_highlights_owned_allocation_declaration_heads() {
        let root = build_bundle_root("owned_allocation_declaration_heads");
        let output = run_tree_sitter_query(
            &root,
            &root.join("queries/fol/highlights.scm"),
            &repo_root().join("examples/mem_linked_list_m1/src/main.fol"),
        );

        assert!(
            output.status.success(),
            "tree-sitter highlight query failed for owned declarations:\nstdout:\n{}\nstderr:\n{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
        let stdout = String::from_utf8_lossy(&output.stdout);
        let owned_declarations = stdout
            .lines()
            .filter(|line| line.contains("capture:"))
            .filter(|line| line.contains("keyword"))
            .filter(|line| line.contains("text: `@var`"))
            .count();
        assert_eq!(
            owned_declarations, 2,
            "both owned allocation declarations should capture '@var' as a keyword:\n{stdout}"
        );

        std::fs::remove_dir_all(root).ok();
    }

    #[test]
    fn generated_bundle_highlights_nested_v3_type_operands_and_tilde_var() {
        let root = build_bundle_root("nested_v3_type_operands");
        let query = root.join("queries/fol/highlights.scm");
        let cases = [
            (
                "examples/mem_ptr_unique_m3/src/main.fol",
                "- type.builtin, start: (2, 26), end: (2, 29), text: `int`",
            ),
            (
                "examples/mem_ptr_shared_recursive_m3/src/main.fol",
                "- type, start: (2, 26), end: (2, 30), text: `Node`",
            ),
            (
                "examples/proc_channel_m2/src/main.fol",
                "- type.builtin, start: (2, 27), end: (2, 30), text: `int`",
            ),
            (
                "examples/mem_linked_list_m1/src/main.fol",
                "- type, start: (2, 15), end: (2, 19), text: `Node`",
            ),
            (
                "test/parser/simple_fun_tilde_var.fol",
                "- keyword, start: (1, 4), end: (1, 8), text: `~var`",
            ),
        ];

        for (relative, expected) in cases {
            let output = run_tree_sitter_query(&root, &query, &repo_root().join(relative));
            assert!(
                output.status.success(),
                "tree-sitter highlight query failed for '{relative}':\nstdout:\n{}\nstderr:\n{}",
                String::from_utf8_lossy(&output.stdout),
                String::from_utf8_lossy(&output.stderr)
            );
            let stdout = String::from_utf8_lossy(&output.stdout);
            assert!(
                stdout.lines().any(|line| line.contains(expected)),
                "fixture '{relative}' lost exact nested V3 capture '{expected}':\n{stdout}"
            );
        }

        std::fs::remove_dir_all(root).ok();
    }

    #[test]
    fn generated_bundle_highlights_recursive_v3_type_operands_exactly() {
        let root = build_bundle_root("recursive_v3_type_operands");
        let source = root.join("recursive_v3_type_operands.fol");
        std::fs::write(
            &source,
            concat!(
                "typ Box(T): rec = { value: T };\n",
                "use models: loc = {\"../models\"};\n",
                "fun[] demo(\n",
                "    a: ptr[Box[int]],\n",
                "    b: chn[vec[Node]],\n",
                "    c: @arr[Node, 1],\n",
                "    d: ptr[opt[Node]],\n",
                "    e: ptr[models::Box[int]],\n",
                "    f: chn[vec[models::Node]],\n",
                "    g: @arr[models::Node, 1],\n",
                "    h: ptr[opt[models::Node]]\n",
                "): int = { return 0; };\n",
            ),
        )
        .expect("recursive V3 type fixture should be writable");

        let output =
            run_tree_sitter_query(&root, &root.join("queries/fol/highlights.scm"), &source);
        assert!(
            output.status.success(),
            "tree-sitter highlight query failed for recursive V3 types:\nstdout:\n{}\nstderr:\n{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
        let stdout = String::from_utf8_lossy(&output.stdout);
        let expected = [
            ("type", 3, 11, 14, "Box"),
            ("type.builtin", 3, 15, 18, "int"),
            ("type", 4, 15, 19, "Node"),
            ("type", 5, 12, 16, "Node"),
            ("type", 6, 15, 19, "Node"),
            ("type", 7, 11, 22, "models::Box"),
            ("type.builtin", 7, 23, 26, "int"),
            ("type", 8, 15, 27, "models::Node"),
            ("type", 9, 12, 24, "models::Node"),
            ("type", 10, 15, 27, "models::Node"),
        ];

        for (capture, row, start, end, text) in expected {
            let exact = format!(
                "- {capture}, start: ({row}, {start}), end: ({row}, {end}), text: `{text}`"
            );
            assert_eq!(
                stdout.lines().filter(|line| line.contains(&exact)).count(),
                1,
                "recursive V3 operand should have one exact '{exact}' capture:\n{stdout}"
            );
        }

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
        assert!(stdout.contains("local.definition.type"));
        assert!(stdout.contains("local.definition.method"));
        assert!(stdout.contains("local.definition.function"));
        assert!(stdout.contains("local.definition"));

        std::fs::remove_dir_all(root).ok();
    }

    #[test]
    fn locals_query_scopes_routines_anonymous_routines_and_select_arms() {
        let query = fol_tree_sitter_locals_query();
        for node in [
            "plain_fun_decl",
            "plain_pro_decl",
            "plain_log_decl",
            "method_decl",
            "anonymous_fun_expr",
            "anonymous_pro_expr",
            "anonymous_log_expr",
            "select_arm",
        ] {
            let expected = format!("({node}) @local.scope");
            assert!(
                query.contains(&expected),
                "locals query should isolate '{node}' bindings with '{expected}'"
            );
        }
    }

    #[test]
    fn generated_bundle_locals_query_scopes_v3_parameters_and_select_binders() {
        let root = build_bundle_root("v3_parameter_and_select_scopes");
        let cases = [
            (
                "examples/proc_mutex_m3/src/main.fol",
                [
                    "capture: local.scope, start: (6, 6), end: (10, 1)",
                    "capture: local.scope, start: (12, 6), end: (16, 1)",
                ]
                .as_slice(),
            ),
            (
                "examples/proc_channel_capture_m2/src/main.fol",
                ["capture: local.scope, start: (4, 7), end: (6, 5)"].as_slice(),
            ),
            (
                "examples/proc_select_m3/src/main.fol",
                [
                    "capture: local.scope, start: (13, 8), end: (15, 9)",
                    "capture: local.scope, start: (16, 8), end: (18, 9)",
                    "capture: local.scope, start: (21, 8), end: (23, 9)",
                ]
                .as_slice(),
            ),
        ];

        for (relative, expected_scopes) in cases {
            let output = run_tree_sitter_query(
                &root,
                &root.join("queries/fol/locals.scm"),
                &repo_root().join(relative),
            );
            assert!(
                output.status.success(),
                "tree-sitter locals query failed for '{relative}':\nstdout:\n{}\nstderr:\n{}",
                String::from_utf8_lossy(&output.stdout),
                String::from_utf8_lossy(&output.stderr)
            );
            let stdout = String::from_utf8_lossy(&output.stdout);
            for expected_scope in expected_scopes {
                assert!(
                    stdout.contains(expected_scope),
                    "fixture '{relative}' lost exact local scope '{expected_scope}':\n{stdout}"
                );
            }
        }

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
        assert!(stdout.contains("symbol.type"));
        assert!(stdout.contains("symbol.method"));
        assert!(stdout.contains("symbol.function"));
        assert!(stdout.contains("symbol.variable"));

        std::fs::remove_dir_all(root).ok();
    }

    #[test]
    fn generated_bundle_queries_structure_channel_loop_iteration_binders() {
        let root = build_bundle_root("channel_loop_iteration_binder");
        let source = repo_root().join("examples/proc_channel_loop_m2/src/main.fol");

        for (query_name, capture, text) in [
            ("highlights", "operator", "in"),
            ("highlights", "variable", "value"),
            ("locals", "local.definition", "value"),
            ("symbols", "symbol.variable", "value"),
        ] {
            let output = run_tree_sitter_query(
                &root,
                &root.join(format!("queries/fol/{query_name}.scm")),
                &source,
            );
            assert!(
                output.status.success(),
                "tree-sitter {query_name} query failed for channel loop:\nstdout:\n{}\nstderr:\n{}",
                String::from_utf8_lossy(&output.stdout),
                String::from_utf8_lossy(&output.stderr)
            );
            let stdout = String::from_utf8_lossy(&output.stdout);
            assert!(
                stdout.lines().any(|line| {
                    line.contains(capture) && line.contains(&format!("text: `{text}`"))
                }),
                "channel loop query '{query_name}' lost {capture} capture for '{text}':\n{stdout}"
            );
        }

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
            assert!(
                stdout.contains("namespace"),
                "expected namespace captures for '{}':\n{}",
                relative,
                stdout
            );
            assert!(
                stdout.contains("function"),
                "expected function captures for '{}':\n{}",
                relative,
                stdout
            );
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
        for needle in ["keyword.conditional", "keyword.return", "keyword.exception"] {
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
        ] {
            assert!(
                query.contains(needle),
                "missing source-kind capture: {needle}"
            );
        }
        assert!(
            !query.contains("(source_kind \"std\""),
            "removed std source kind must not remain in the highlight query"
        );

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
        for needle in ["namespace", "keyword.function", "keyword.return"] {
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
        for needle in ["type.builtin", "constant.builtin", "punctuation.bracket"] {
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
                repo_root().join("test/apps/fixtures/dfr_scope_exit/main.fol"),
                ["keyword.repeat", "punctuation.bracket"].as_slice(),
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
                repo_root().join("examples/generic_routine_cross_file_m1/src/main.fol"),
                ["function", "type.builtin"].as_slice(),
            ),
            (
                repo_root().join("examples/generic_routine_cross_file_m1/src/shared.fol"),
                ["function", "type"].as_slice(),
            ),
            (
                repo_root().join("examples/memo_run_min/src/main.fol"),
                ["function", "type.builtin"].as_slice(),
            ),
            (
                repo_root().join("examples/generic_type_semantic_m1m2/src/main.fol"),
                ["type", "function"].as_slice(),
            ),
            (
                repo_root().join("examples/generic_type_exec_m1m2/src/main.fol"),
                ["keyword.import", "type", "function"].as_slice(),
            ),
            (
                repo_root().join("examples/fail_generic_misuse_m1/src/main.fol"),
                ["type", "function"].as_slice(),
            ),
            (
                repo_root().join("examples/fail_generic_standard_constraint_m1m2/src/main.fol"),
                ["type", "function"].as_slice(),
            ),
            (
                repo_root().join("examples/core_dfr/src/main.fol"),
                ["keyword.repeat", "type.builtin"].as_slice(),
            ),
            (
                repo_root().join("examples/mem_linked_list_m1/src/main.fol"),
                ["operator", "type", "type.builtin"].as_slice(),
            ),
            (
                repo_root().join("examples/mem_tree_m1/src/main.fol"),
                ["operator", "type", "type.builtin"].as_slice(),
            ),
            (
                repo_root().join("examples/mem_move_stack_vs_heap_m1/src/main.fol"),
                ["operator", "type.builtin"].as_slice(),
            ),
            (
                repo_root().join("examples/fail_mem_use_after_move_m1/src/main.fol"),
                ["operator", "type.builtin"].as_slice(),
            ),
            (
                repo_root().join("examples/fail_mem_recursive_value_m1/src/main.fol"),
                ["type", "function"].as_slice(),
            ),
            (
                repo_root().join("examples/fail_mem_heap_in_core_m1/src/main.fol"),
                ["operator", "type.builtin"].as_slice(),
            ),
            (
                repo_root().join("examples/mem_borrow_m2/src/main.fol"),
                ["operator", "type", "variable"].as_slice(),
            ),
            (
                repo_root().join("examples/mem_borrow_giveback_m2/src/main.fol"),
                ["operator", "type", "variable"].as_slice(),
            ),
            (
                repo_root().join("examples/mem_borrow_param_m2/src/main.fol"),
                ["operator", "attribute", "variable.parameter"].as_slice(),
            ),
            (
                repo_root().join("examples/mem_mut_borrow_m2/src/main.fol"),
                ["operator", "type", "variable"].as_slice(),
            ),
            (
                repo_root().join("examples/mem_edf_m2/src/main.fol"),
                ["keyword.exception", "keyword.repeat", "function"].as_slice(),
            ),
            (
                repo_root().join("examples/fail_mem_owner_while_borrowed_m2/src/main.fol"),
                ["type.builtin", "variable"].as_slice(),
            ),
            (
                repo_root().join("examples/fail_mem_second_mut_borrow_m2/src/main.fol"),
                ["type.builtin", "variable"].as_slice(),
            ),
            (
                repo_root().join("examples/fail_mem_mut_borrow_immutable_owner_m2/src/main.fol"),
                ["type.builtin", "variable"].as_slice(),
            ),
            (
                repo_root().join("examples/mem_ptr_unique_m3/src/main.fol"),
                ["type.builtin", "operator", "variable"].as_slice(),
            ),
            (
                repo_root().join("examples/mem_ptr_shared_m3/src/main.fol"),
                ["type.builtin", "attribute", "operator"].as_slice(),
            ),
            (
                repo_root().join("examples/mem_ptr_shared_recursive_m3/src/main.fol"),
                ["type.builtin", "attribute", "type"].as_slice(),
            ),
            (
                repo_root().join("examples/proc_spawn_m1/src/main.fol"),
                ["keyword", "function"].as_slice(),
            ),
            (
                repo_root().join("examples/proc_spawn_move_heap_m1/src/main.fol"),
                ["keyword", "operator", "function"].as_slice(),
            ),
            (
                repo_root().join("examples/proc_channel_m2/src/main.fol"),
                ["type.builtin", "attribute", "operator"].as_slice(),
            ),
            (
                repo_root().join("examples/proc_channel_pull_m2/src/main.fol"),
                ["type.builtin", "attribute", "operator"].as_slice(),
            ),
            (
                repo_root().join("examples/proc_channel_capture_m2/src/main.fol"),
                ["type.builtin", "attribute", "keyword"].as_slice(),
            ),
            (
                repo_root().join("examples/proc_channel_loop_m2/src/main.fol"),
                ["type.builtin", "attribute", "keyword.repeat"].as_slice(),
            ),
            (
                repo_root().join("examples/proc_select_m3/src/main.fol"),
                ["keyword.conditional", "variable", "type.builtin"].as_slice(),
            ),
            (
                repo_root().join("examples/proc_mutex_m3/src/main.fol"),
                ["attribute", "function", "property"].as_slice(),
            ),
            (
                repo_root().join("examples/proc_mutex_explicit_unlock_m3/src/main.fol"),
                ["attribute", "function", "property"].as_slice(),
            ),
            (
                repo_root().join("examples/proc_async_await_m4/src/main.fol"),
                ["keyword", "function", "type.builtin"].as_slice(),
            ),
            (
                repo_root().join("examples/proc_await_error_m4/src/main.fol"),
                ["keyword", "function", "operator"].as_slice(),
            ),
            (
                repo_root().join("examples/fail_mem_ptr_raw_m3/src/main.fol"),
                ["type.builtin", "attribute"].as_slice(),
            ),
            (
                repo_root().join("examples/fail_mem_ptr_in_core_m3/src/main.fol"),
                ["type.builtin", "operator"].as_slice(),
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
                repo_root().join("examples/standards_protocol_multi_m2/src/main.fol"),
                ["function"].as_slice(),
            ),
            (
                repo_root().join("examples/standards_protocol_multi_m2/src/contracts.fol"),
                ["function", "type"].as_slice(),
            ),
            (
                repo_root().join("examples/standards_protocol_multi_m2/src/rect.fol"),
                ["function", "type"].as_slice(),
            ),
            (
                repo_root().join("examples/fail_standard_blueprint_m2/src/main.fol"),
                ["type", "keyword.type"].as_slice(),
            ),
            (
                repo_root().join("examples/fail_standard_as_type_m2/src/main.fol"),
                ["function", "type"].as_slice(),
            ),
            (
                repo_root().join("examples/fail_standard_missing_routine_m2/src/main.fol"),
                ["function", "type"].as_slice(),
            ),
            (
                repo_root().join("examples/fail_standard_signature_m2/src/main.fol"),
                ["function", "type"].as_slice(),
            ),
            (
                repo_root().join("examples/fail_standard_import_ambiguity_m2/src/main.fol"),
                ["function", "type"].as_slice(),
            ),
            (
                repo_root().join("examples/generic_turbofish_m1/src/main.fol"),
                ["keyword.import", "function"].as_slice(),
            ),
            (
                repo_root().join("examples/generic_type_constrained_m1m2/src/main.fol"),
                ["keyword.import", "function", "type"].as_slice(),
            ),
            (
                repo_root().join("examples/generic_error_m1m2/src/main.fol"),
                ["keyword.import", "function"].as_slice(),
            ),
            (
                repo_root().join("examples/standards_default_body_m2/src/main.fol"),
                ["keyword.import", "function", "type"].as_slice(),
            ),
            (
                repo_root().join("examples/standards_blueprint_m2/src/main.fol"),
                ["keyword.import", "function", "type"].as_slice(),
            ),
            (
                repo_root().join("examples/standards_extended_m2/src/main.fol"),
                ["keyword.import", "function", "type"].as_slice(),
            ),
            (
                repo_root().join("examples/standards_generic_m2/src/main.fol"),
                ["keyword.import", "function", "type"].as_slice(),
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
                ["function", "operator"].as_slice(),
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

    #[test]
    fn v3_examples_keep_zero_error_trees_and_dead_forms_stay_dead() {
        let root = build_bundle_root("v3_zero_error_trees");
        let cache = tree_sitter_cache_root("v3_zero_error_trees");
        let repo = repo_root();
        let mut sources = BTreeSet::new();
        for relative_root in [
            "examples",
            "lang/library/std",
            "test/apps/showcases",
            "xtra",
        ] {
            collect_fol_sources(&repo.join(relative_root), &mut sources);
        }

        // These are deliberately invalid grammar fixtures. Their build files
        // and every other example source remain in the zero-ERROR sweep.
        let intentionally_invalid_syntax = [
            "examples/fail_proc_select_old_form_m3/src/main.fol",
            "examples/fail_proc_mutex_double_paren_m3/src/main.fol",
        ];
        for relative in intentionally_invalid_syntax {
            let path = repo.join(relative);
            assert!(
                sources.remove(&path),
                "explicit invalid-syntax fixture '{relative}' should exist in the source inventory"
            );
        }

        for required in [
            "examples/core_run_min/build.fol",
            "examples/core_run_min/src/main.fol",
            "examples/mem_ptr_unique_m3/src/main.fol",
            "examples/proc_async_await_m4/src/main.fol",
            "lang/library/std/build.fol",
            "lang/library/std/io/lib.fol",
            "test/apps/showcases/full_v1_showcase/app/main.fol",
            "xtra/logtiny/build.fol",
            "xtra/logtiny/src/log.fol",
        ] {
            assert!(
                sources.contains(&repo.join(required)),
                "checked-in syntax inventory should include '{required}'"
            );
        }
        assert!(
            sources.len() > 250,
            "checked-in syntax inventory unexpectedly shrank to {} files",
            sources.len()
        );

        let sources = sources.into_iter().collect::<Vec<_>>();
        let output = run_tree_sitter_parse_many(&root, &cache, &sources);
        assert!(
            output.status.success(),
            "{} checked-in FOL sources should parse without ERROR nodes:\nstdout:\n{}\nstderr:\n{}",
            sources.len(),
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );

        for relative in intentionally_invalid_syntax {
            let source = repo.join(relative);
            let output = run_tree_sitter_parse(&root, &cache, &source);
            let stdout = String::from_utf8_lossy(&output.stdout);
            assert!(
                !output.status.success() && stdout.contains("(ERROR"),
                "dead V3 form '{relative}' should retain an ERROR node:\nstdout:\n{}\nstderr:\n{}",
                stdout,
                String::from_utf8_lossy(&output.stderr)
            );
        }

        for (name, source) in [
            (
                "legacy_defer_block.fol",
                "pro[] main(): non = {\n    defer { return; };\n};\n",
            ),
            (
                "legacy_go_block.fol",
                "pro[] main(): non = {\n    go { return; };\n};\n",
            ),
            (
                "dead_var_tilde_option.fol",
                "pro[] main(): non = {\n    var[~] value: int = 1;\n    return;\n};\n",
            ),
        ] {
            let path = root.join(name);
            std::fs::write(&path, source).expect("legacy-form fixture should be writable");
            let output = run_tree_sitter_parse(&root, &cache, &path);
            let stdout = String::from_utf8_lossy(&output.stdout);
            assert!(
                !output.status.success() && stdout.contains("(ERROR"),
                "deleted legacy form '{name}' should retain an ERROR node:\nstdout:\n{}\nstderr:\n{}",
                stdout,
                String::from_utf8_lossy(&output.stderr)
            );
        }

        let reserved_prefixes = root.join("reserved_prefix_identifiers.fol");
        std::fs::write(
            &reserved_prefixes,
            "pro[] deferred(goal: int): int = {\n    return goal;\n};\n",
        )
        .expect("reserved-prefix fixture should be writable");
        let output = run_tree_sitter_parse(&root, &cache, &reserved_prefixes);
        let stdout = String::from_utf8_lossy(&output.stdout);
        assert!(
            output.status.success() && !stdout.contains("(ERROR"),
            "only the exact deleted words should be reserved:\nstdout:\n{}\nstderr:\n{}",
            stdout,
            String::from_utf8_lossy(&output.stderr)
        );

        std::fs::remove_dir_all(root).ok();
        std::fs::remove_dir_all(cache).ok();
    }
}
