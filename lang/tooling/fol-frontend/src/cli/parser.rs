use crate::ansi::Colored;
use crate::OutputMode;

use super::args::*;

const VERSION: &str = env!("CARGO_PKG_VERSION");
const AFTER_HELP: &str = "Run `fol <group> <command> --help` for command-specific usage.";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FrontendCli {
    pub input: Option<String>,
    pub output: OutputMode,
    pub json: bool,
    pub profile: Option<FrontendProfile>,
    pub debug: bool,
    pub release: bool,
    pub std_root: Option<String>,
    pub package_store_root: Option<String>,
    pub dump_lowered: bool,
    pub emit_rust: bool,
    pub keep_build_dir: bool,
    pub command: Option<FrontendCommand>,
}

impl FrontendCli {
    pub fn selected_profile(&self) -> FrontendProfile {
        if self.release {
            FrontendProfile::Release
        } else if self.debug {
            FrontendProfile::Debug
        } else {
            self.profile.unwrap_or(FrontendProfile::Debug)
        }
    }
}

// ---------------------------------------------------------------------------
// Parse error
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ParseErrorKind {
    Help(String),
    Version,
    InvalidInput(String),
    InvalidSubcommand(String),
    Conflict(String),
    MissingValue(String),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParseError {
    pub kind: ParseErrorKind,
}

impl ParseError {
    fn help(text: String) -> Self {
        Self {
            kind: ParseErrorKind::Help(text),
        }
    }
    fn version() -> Self {
        Self {
            kind: ParseErrorKind::Version,
        }
    }
    fn invalid(msg: impl Into<String>) -> Self {
        Self {
            kind: ParseErrorKind::InvalidInput(msg.into()),
        }
    }
    fn invalid_subcommand(msg: impl Into<String>) -> Self {
        Self {
            kind: ParseErrorKind::InvalidSubcommand(msg.into()),
        }
    }
    fn conflict(msg: impl Into<String>) -> Self {
        Self {
            kind: ParseErrorKind::Conflict(msg.into()),
        }
    }
    fn missing(msg: impl Into<String>) -> Self {
        Self {
            kind: ParseErrorKind::MissingValue(msg.into()),
        }
    }
}

impl std::fmt::Display for ParseError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match &self.kind {
            ParseErrorKind::Help(text) => write!(f, "{text}"),
            ParseErrorKind::Version => write!(f, "fol-frontend {VERSION}"),
            ParseErrorKind::InvalidInput(msg)
            | ParseErrorKind::InvalidSubcommand(msg)
            | ParseErrorKind::Conflict(msg)
            | ParseErrorKind::MissingValue(msg) => write!(f, "{msg}"),
        }
    }
}

// ---------------------------------------------------------------------------
// Arg cursor helper
// ---------------------------------------------------------------------------

struct ArgCursor {
    args: Vec<String>,
    pos: usize,
}

impl ArgCursor {
    fn new(args: Vec<String>) -> Self {
        Self { args, pos: 0 }
    }

    fn peek(&self) -> Option<&str> {
        self.args.get(self.pos).map(|s| s.as_str())
    }

    fn advance(&mut self) -> Option<&str> {
        let val = self.args.get(self.pos).map(|s| s.as_str());
        if val.is_some() {
            self.pos += 1;
        }
        val
    }

    fn remaining_as_vec(&self) -> Vec<String> {
        self.args[self.pos..].to_vec()
    }

    fn is_done(&self) -> bool {
        self.pos >= self.args.len()
    }

    /// Take the value for a --flag. Handles both `--flag value` and `--flag=value`.
    /// The `current` is the already-consumed token. If it contains '=', return the part after '='.
    /// Otherwise consume the next token.
    fn take_value(&mut self, current: &str, flag_name: &str) -> Result<String, ParseError> {
        if let Some(eq_pos) = current.find('=') {
            Ok(current[eq_pos + 1..].to_string())
        } else {
            self.advance()
                .map(|s| s.to_string())
                .ok_or_else(|| ParseError::missing(format!("--{flag_name} requires a value")))
        }
    }
}

// ---------------------------------------------------------------------------
// Public parse interface
// ---------------------------------------------------------------------------

impl FrontendCli {
    pub fn parse_from<I, T>(args: I) -> Self
    where
        I: IntoIterator<Item = T>,
        T: Into<String>,
    {
        match Self::try_parse_from(args) {
            Ok(cli) => cli,
            Err(e) => {
                eprintln!("{e}");
                std::process::exit(match e.kind {
                    ParseErrorKind::Help(_) | ParseErrorKind::Version => 0,
                    _ => 1,
                });
            }
        }
    }

    pub fn try_parse_from<I, T>(args: I) -> Result<Self, ParseError>
    where
        I: IntoIterator<Item = T>,
        T: Into<String>,
    {
        let args: Vec<String> = args.into_iter().map(|a| a.into()).collect();
        // Skip argv[0] (the binary name)
        let args = if !args.is_empty() {
            args[1..].to_vec()
        } else {
            vec![]
        };
        parse_root(args)
    }
}

// ---------------------------------------------------------------------------
// Help text
// ---------------------------------------------------------------------------

/// Style a name and pad it to `width` visible characters (padding outside ANSI codes).
fn cmd(name: &str, width: usize) -> String {
    let spaces = width.saturating_sub(name.len());
    format!("{}{}", name.bold(), " ".repeat(spaces))
}

/// Style an alias string with dim.
fn alias(s: &str) -> String {
    format!("{}", s.dim())
}

/// Style an option flag and pad to `width` visible characters.
fn opt(flag: &str, width: usize) -> String {
    let spaces = width.saturating_sub(flag.len());
    format!("{}{}", flag.bold(), " ".repeat(spaces))
}

fn section(s: &str) -> String {
    format!("{}", s.yellow().bold())
}

fn root_help() -> String {
    let s = section;
    format!(
        "\
User-facing frontend for the FOL toolchain

{usage} fol [COMMAND]

{cmds}
  {work}  {aw}  Workspace management
  {pack}  {ap}  Package management
  {code}  {ac}  Build, run, test, check
  {tool}  {at}  Editor tools, LSP, completion

{opts}
  {h}, {hh}     Print help
  {v}, {vv}  Print version

{after}",
        usage = s("Usage:"),
        cmds = s("Commands:"),
        opts = s("Options:"),
        work = cmd("work", 4),
        pack = cmd("pack", 4),
        code = cmd("code", 4),
        tool = cmd("tool", 4),
        aw = alias("[aliases: w]"),
        ap = alias("[aliases: p]"),
        ac = alias("[aliases: c]"),
        at = alias("[aliases: t]"),
        h = opt("-h", 2),
        hh = opt("--help", 6),
        v = opt("-V", 2),
        vv = opt("--version", 9),
        after = AFTER_HELP.dim(),
    )
}

fn work_help() -> String {
    let s = section;
    format!(
        "\
{usage} fol work [OPTIONS] <COMMAND>

{cmds}
  {c0}  Initialize a new package or workspace
  {c1}  Create a new project
  {c2}  Show workspace info
  {c3}  List workspace members
  {c4}  Show dependency tree
  {c5}  Show workspace status

{opts}
  {o0}  Select output mode [human|plain|json]
  {o1}  Override the workspace or package root
  {o2}, {o3}  Print help",
        usage = s("Usage:"),
        cmds = s("Commands:"),
        opts = s("Options:"),
        c0 = cmd("init", 6),
        c1 = cmd("new", 6),
        c2 = cmd("info", 6),
        c3 = cmd("list", 6),
        c4 = cmd("deps", 6),
        c5 = cmd("status", 6),
        o0 = opt("--output <MODE>", 15),
        o1 = opt("--path <PATH>", 15),
        o2 = opt("-h", 2),
        o3 = opt("--help", 6),
    )
}

fn pack_help() -> String {
    let s = section;
    format!(
        "\
{usage} fol pack [OPTIONS] <COMMAND>

{cmds}
  {c0}  {a0}  Fetch dependencies
  {c1}  {a1}  Update dependencies

{opts}
  {o0}  Select output mode [human|plain|json]
  {o1}, {o2}  Print help",
        usage = s("Usage:"),
        cmds = s("Commands:"),
        opts = s("Options:"),
        c0 = cmd("fetch", 6),
        c1 = cmd("update", 6),
        a0 = alias("[aliases: f, sync]   "),
        a1 = alias("[aliases: u, upgrade]"),
        o0 = opt("--output <MODE>", 15),
        o1 = opt("-h", 2),
        o2 = opt("--help", 6),
    )
}

fn code_help() -> String {
    let s = section;
    format!(
        "\
{usage} fol code [OPTIONS] <COMMAND>

{cmds}
  {c0}  {a0}  Build the project
  {c1}  {a1}  Build and run the project
  {c2}  {a2}  Run tests
  {c3}  {a3}  Check without building
  {c4}  {a4}  Emit intermediate representations
  {c5}          Explain a diagnostic code

{opts}
  {o0}  Select output mode [human|plain|json]
  {o1}  Select build profile [debug|release]
  {o2}  Force the debug profile
  {o3}  Force the release profile
  {o4}, {o5}  Print help",
        usage = s("Usage:"),
        cmds = s("Commands:"),
        opts = s("Options:"),
        c0 = cmd("build", 5),
        c1 = cmd("run", 5),
        c2 = cmd("test", 5),
        c3 = cmd("check", 5),
        c4 = cmd("emit", 5),
        c5 = cmd("explain", 5),
        a0 = alias("[aliases: b, make]  "),
        a1 = alias("[aliases: r]        "),
        a2 = alias("[aliases: t]        "),
        a3 = alias("[aliases: c, verify]"),
        a4 = alias("[aliases: e, gen]   "),
        o0 = opt("--output <MODE>", 16),
        o1 = opt("--profile <PROF>", 16),
        o2 = opt("--debug", 16),
        o3 = opt("--release", 16),
        o4 = opt("-h", 2),
        o5 = opt("--help", 6),
    )
}

fn tool_help() -> String {
    let s = section;
    format!(
        "\
{usage} fol tool [OPTIONS] <COMMAND>

{cmds}
  {c0}  Start the LSP server
  {c1}  Format a source file
  {c2}  Parse and dump a source file
  {c3}  Highlight a source file
  {c4}  List symbols in a source file
  {c5}  Find references to a symbol
  {c6}  Rename a symbol
  {c7}  Get completions at a position
  {c8}  Get semantic tokens for a file
  {c9}  Tree-sitter commands
  {ca}  {aa}  Clean build artifacts
  {cb}  {ab}  Generate shell completion

{opts}
  {o0}  Select output mode [human|plain|json]
  {o1}, {o2}  Print help",
        usage = s("Usage:"),
        cmds = s("Commands:"),
        opts = s("Options:"),
        c0 = cmd("lsp", 15),
        c1 = cmd("format", 15),
        c2 = cmd("parse", 15),
        c3 = cmd("highlight", 15),
        c4 = cmd("symbols", 15),
        c5 = cmd("references", 15),
        c6 = cmd("rename", 15),
        c7 = cmd("complete", 15),
        c8 = cmd("semantic-tokens", 15),
        c9 = cmd("tree", 15),
        ca = cmd("clean", 15),
        cb = cmd("completion", 15),
        aa = alias("[aliases: cl, purge]        "),
        ab = alias("[aliases: completions, comp]"),
        o0 = opt("--output <MODE>", 15),
        o1 = opt("-h", 2),
        o2 = opt("--help", 6),
    )
}

fn emit_help() -> String {
    let s = section;
    format!(
        "\
{usage} fol code emit <COMMAND>

{cmds}
  {c0}  Emit generated Rust code
  {c1}  Emit lowered IR

{opts}
  {o0}, {o1}  Print help",
        usage = s("Usage:"),
        cmds = s("Commands:"),
        opts = s("Options:"),
        c0 = cmd("rust", 7),
        c1 = cmd("lowered", 7),
        o0 = opt("-h", 2),
        o1 = opt("--help", 6),
    )
}

fn tree_help() -> String {
    let s = section;
    format!(
        "\
{usage} fol tool tree <COMMAND>

{cmds}
  {c0}  Generate tree-sitter grammar

{opts}
  {o0}, {o1}  Print help",
        usage = s("Usage:"),
        cmds = s("Commands:"),
        opts = s("Options:"),
        c0 = cmd("generate", 8),
        o0 = opt("-h", 2),
        o1 = opt("--help", 6),
    )
}

fn explain_help() -> String {
    let s = section;
    format!(
        "\
Explain a diagnostic code in plain language

{usage} fol code explain <CODE>

{args}
  {a0}  A diagnostic code, e.g. T1003 (case-insensitive)

{opts}
  {o0}  Select output mode [human|plain|json]
  {o1}  Shorthand for --output json
  {o2}, {o3}  Print help

{ex}
  fol code explain T1003
  fol code explain t1003
  fol code explain --output json R1003",
        usage = s("Usage:"),
        args = s("Arguments:"),
        opts = s("Options:"),
        ex = s("Examples:"),
        a0 = cmd("<CODE>", 15),
        o0 = opt("--output <MODE>", 15),
        o1 = opt("--json", 15),
        o2 = opt("-h", 2),
        o3 = opt("--help", 6),
    )
}

// ---------------------------------------------------------------------------
// Root parser
// ---------------------------------------------------------------------------

fn parse_root(args: Vec<String>) -> Result<FrontendCli, ParseError> {
    let mut cursor = ArgCursor::new(args);

    let mut cli = FrontendCli {
        input: None,
        output: env_output_mode(),
        json: false,
        profile: env_profile(),
        debug: false,
        release: false,
        std_root: None,
        package_store_root: None,
        dump_lowered: false,
        emit_rust: false,
        keep_build_dir: false,
        command: None,
    };

    // Consume root-level flags and find the command
    while let Some(token) = cursor.peek() {
        if token == "--help" || token == "-h" {
            return Err(ParseError::help(root_help()));
        }
        if token == "--version" || token == "-V" {
            return Err(ParseError::version());
        }
        if token.starts_with("--") {
            let token = cursor.advance().unwrap().to_string();
            parse_root_flag(&mut cli, &token, &mut cursor)?;
            continue;
        }
        // Not a flag — must be a command or direct input
        break;
    }

    if cursor.is_done() {
        return Ok(cli);
    }

    let token = cursor.peek().unwrap().to_string();
    match resolve_command_group(&token) {
        Some(group) => {
            cursor.advance();
            cli.command = Some(parse_command_group(group, &mut cursor)?);
        }
        None if token == "_complete" => {
            cursor.advance();
            cli.command = Some(FrontendCommand::Complete(CompleteCommand {
                tokens: cursor.remaining_as_vec(),
            }));
        }
        None => {
            // Treat as direct file/folder input
            cli.input = Some(cursor.advance().unwrap().to_string());
            // Consume any remaining root flags after input
            while let Some(token) = cursor.peek() {
                if token.starts_with("--") {
                    let token = cursor.advance().unwrap().to_string();
                    parse_root_flag(&mut cli, &token, &mut cursor)?;
                } else {
                    break;
                }
            }
        }
    }

    Ok(cli)
}

fn parse_root_flag(
    cli: &mut FrontendCli,
    token: &str,
    cursor: &mut ArgCursor,
) -> Result<(), ParseError> {
    let (key, _) = split_eq(token);
    match key {
        "--output" => cli.output = parse_output_mode(&cursor.take_value(token, "output")?)?,
        "--json" => cli.json = true,
        "--profile" => cli.profile = Some(parse_profile(&cursor.take_value(token, "profile")?)?),
        "--debug" => cli.debug = true,
        "--release" => cli.release = true,
        "--std-root" => cli.std_root = Some(cursor.take_value(token, "std-root")?),
        "--package-store-root" => {
            cli.package_store_root = Some(cursor.take_value(token, "package-store-root")?)
        }
        "--dump-lowered" => cli.dump_lowered = true,
        "--emit-rust" => cli.emit_rust = true,
        "--keep-build-dir" => cli.keep_build_dir = true,
        _ => return Err(ParseError::invalid(format!("unknown flag: {key}"))),
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// Command group parsing
// ---------------------------------------------------------------------------

enum CommandGroup {
    Work,
    Pack,
    Code,
    Tool,
}

fn resolve_command_group(token: &str) -> Option<CommandGroup> {
    match token {
        "work" | "w" => Some(CommandGroup::Work),
        "pack" | "p" => Some(CommandGroup::Pack),
        "code" | "c" => Some(CommandGroup::Code),
        "tool" | "t" => Some(CommandGroup::Tool),
        _ => None,
    }
}

fn parse_command_group(
    group: CommandGroup,
    cursor: &mut ArgCursor,
) -> Result<FrontendCommand, ParseError> {
    match group {
        CommandGroup::Work => parse_work_command(cursor),
        CommandGroup::Pack => parse_pack_command(cursor),
        CommandGroup::Code => parse_code_command(cursor),
        CommandGroup::Tool => parse_tool_command(cursor),
    }
}

// ---------------------------------------------------------------------------
// work
// ---------------------------------------------------------------------------

fn parse_work_command(cursor: &mut ArgCursor) -> Result<FrontendCommand, ParseError> {
    let mut output = FrontendOutputArgs::default();
    let mut path: Option<String> = None;

    // Consume group-level flags
    while let Some(token) = cursor.peek() {
        if token == "--help" || token == "-h" {
            return Err(ParseError::help(work_help()));
        }
        if token.starts_with("--") {
            let token = cursor.advance().unwrap().to_string();
            let (key, _) = split_eq(&token);
            match key {
                "--output" => {
                    output.output = Some(parse_output_mode(&cursor.take_value(&token, "output")?)?)
                }
                "--path" => path = Some(cursor.take_value(&token, "path")?),
                _ => return Err(ParseError::invalid(format!("unknown flag for work: {key}"))),
            }
            continue;
        }
        break;
    }

    let sub = cursor
        .advance()
        .ok_or_else(|| ParseError::help(work_help()))?;
    let subcommand = match sub {
        "init" => WorkSubcommand::Init(parse_init_command(cursor)?),
        "new" => WorkSubcommand::New(parse_new_command(cursor)?),
        "info" => WorkSubcommand::Info(UnitCommand),
        "list" => WorkSubcommand::List(UnitCommand),
        "deps" => WorkSubcommand::Deps(UnitCommand),
        "status" => WorkSubcommand::Status(UnitCommand),
        _ => {
            return Err(ParseError::invalid_subcommand(format!(
                "unknown work subcommand: {sub}"
            )))
        }
    };

    Ok(FrontendCommand::Work(WorkCommand {
        output,
        path,
        command: subcommand,
    }))
}

fn parse_init_command(cursor: &mut ArgCursor) -> Result<InitCommand, ParseError> {
    let mut cmd = InitCommand::default();
    while let Some(token) = cursor.peek() {
        match token {
            "--workspace" => {
                cursor.advance();
                cmd.workspace = true;
            }
            "--bin" => {
                cursor.advance();
                cmd.bin = true;
            }
            "--lib" => {
                cursor.advance();
                cmd.lib = true;
            }
            "-h" | "--help" => {
                return Err(ParseError::help(
                    "Usage: fol work init [--workspace] [--bin|--lib]".to_string(),
                ))
            }
            _ => {
                return Err(ParseError::invalid(format!(
                    "unknown flag for init: {token}"
                )))
            }
        }
    }
    if cmd.bin && cmd.lib {
        return Err(ParseError::conflict(
            "--bin and --lib cannot be used together",
        ));
    }
    Ok(cmd)
}

fn parse_new_command(cursor: &mut ArgCursor) -> Result<NewCommand, ParseError> {
    let mut workspace = false;
    let mut bin = false;
    let mut lib = false;
    let mut name: Option<String> = None;

    while let Some(token) = cursor.peek() {
        match token {
            "--workspace" => {
                cursor.advance();
                workspace = true;
            }
            "--bin" => {
                cursor.advance();
                bin = true;
            }
            "--lib" => {
                cursor.advance();
                lib = true;
            }
            "-h" | "--help" => {
                return Err(ParseError::help(
                    "Usage: fol work new <NAME> [--workspace] [--bin|--lib]".to_string(),
                ))
            }
            t if t.starts_with('-') => {
                return Err(ParseError::invalid(format!("unknown flag for new: {t}")))
            }
            _ => {
                name = Some(cursor.advance().unwrap().to_string());
            }
        }
    }
    if bin && lib {
        return Err(ParseError::conflict(
            "--bin and --lib cannot be used together",
        ));
    }
    let name = name.ok_or_else(|| ParseError::missing("work new requires a project name"))?;
    Ok(NewCommand {
        name,
        workspace,
        bin,
        lib,
    })
}

// ---------------------------------------------------------------------------
// pack
// ---------------------------------------------------------------------------

fn parse_pack_command(cursor: &mut ArgCursor) -> Result<FrontendCommand, ParseError> {
    let mut output = FrontendOutputArgs::default();

    while let Some(token) = cursor.peek() {
        if token == "--help" || token == "-h" {
            return Err(ParseError::help(pack_help()));
        }
        if token.starts_with("--") {
            let token = cursor.advance().unwrap().to_string();
            let (key, _) = split_eq(&token);
            match key {
                "--output" => {
                    output.output = Some(parse_output_mode(&cursor.take_value(&token, "output")?)?)
                }
                _ => return Err(ParseError::invalid(format!("unknown flag for pack: {key}"))),
            }
            continue;
        }
        break;
    }

    let sub = cursor
        .advance()
        .ok_or_else(|| ParseError::help(pack_help()))?;
    let sub_output = FrontendOutputArgs::default();
    let subcommand = match sub {
        "fetch" | "f" | "sync" => PackSubcommand::Fetch(parse_fetch_command(cursor, sub_output)?),
        "update" | "u" | "upgrade" => {
            PackSubcommand::Update(parse_update_command(cursor, sub_output)?)
        }
        _ => {
            return Err(ParseError::invalid_subcommand(format!(
                "unknown pack subcommand: {sub}"
            )))
        }
    };

    Ok(FrontendCommand::Pack(PackCommand {
        output,
        command: subcommand,
    }))
}

fn parse_fetch_command(
    cursor: &mut ArgCursor,
    output: FrontendOutputArgs,
) -> Result<FetchCommand, ParseError> {
    let mut cmd = FetchCommand {
        output,
        ..FetchCommand::default()
    };
    while let Some(token) = cursor.peek() {
        if token.starts_with("--") {
            let token = cursor.advance().unwrap().to_string();
            let (key, _) = split_eq(&token);
            match key {
                "--output" => {
                    cmd.output.output =
                        Some(parse_output_mode(&cursor.take_value(&token, "output")?)?)
                }
                "--std-root" => cmd.roots.std_root = Some(cursor.take_value(&token, "std-root")?),
                "--package-store-root" => {
                    cmd.roots.package_store_root =
                        Some(cursor.take_value(&token, "package-store-root")?)
                }
                "--locked" => cmd.locked = true,
                "--offline" => cmd.offline = true,
                "--refresh" => cmd.refresh = true,
                "-h" | "--help" => {
                    return Err(ParseError::help(
                        "Usage: fol pack fetch [--locked] [--offline] [--refresh]".to_string(),
                    ))
                }
                _ => {
                    return Err(ParseError::invalid(format!(
                        "unknown flag for fetch: {key}"
                    )))
                }
            }
        } else {
            break;
        }
    }
    Ok(cmd)
}

fn parse_update_command(
    cursor: &mut ArgCursor,
    output: FrontendOutputArgs,
) -> Result<UpdateCommand, ParseError> {
    let mut cmd = UpdateCommand {
        output,
        ..UpdateCommand::default()
    };
    while let Some(token) = cursor.peek() {
        if token.starts_with("--") {
            let token = cursor.advance().unwrap().to_string();
            let (key, _) = split_eq(&token);
            match key {
                "--output" => {
                    cmd.output.output =
                        Some(parse_output_mode(&cursor.take_value(&token, "output")?)?)
                }
                "--std-root" => cmd.roots.std_root = Some(cursor.take_value(&token, "std-root")?),
                "--package-store-root" => {
                    cmd.roots.package_store_root =
                        Some(cursor.take_value(&token, "package-store-root")?)
                }
                "-h" | "--help" => {
                    return Err(ParseError::help("Usage: fol pack update".to_string()))
                }
                _ => {
                    return Err(ParseError::invalid(format!(
                        "unknown flag for update: {key}"
                    )))
                }
            }
        } else {
            break;
        }
    }
    Ok(cmd)
}

// ---------------------------------------------------------------------------
// code
// ---------------------------------------------------------------------------

fn parse_code_command(cursor: &mut ArgCursor) -> Result<FrontendCommand, ParseError> {
    let mut output = FrontendOutputArgs::default();
    let mut profile_args = env_profile_args();

    while let Some(token) = cursor.peek() {
        if token == "--help" || token == "-h" {
            return Err(ParseError::help(code_help()));
        }
        if token.starts_with("--") {
            let token = cursor.advance().unwrap().to_string();
            let (key, _) = split_eq(&token);
            match key {
                "--output" => {
                    output.output = Some(parse_output_mode(&cursor.take_value(&token, "output")?)?)
                }
                "--profile" => {
                    profile_args.profile =
                        Some(parse_profile(&cursor.take_value(&token, "profile")?)?)
                }
                "--debug" => profile_args.debug = true,
                "--release" => profile_args.release = true,
                _ => return Err(ParseError::invalid(format!("unknown flag for code: {key}"))),
            }
            continue;
        }
        break;
    }

    check_profile_conflicts(&profile_args)?;

    let sub = cursor
        .advance()
        .ok_or_else(|| ParseError::help(code_help()))?;
    // Command-local overrides stay sparse so dispatch can resolve
    // subcommand > group > root/environment precedence exactly once.
    let sub_output = FrontendOutputArgs::default();
    let sub_profile = env_profile_args();
    let subcommand = match sub {
        "build" | "b" | "make" => {
            CodeSubcommand::Build(parse_build_command(cursor, sub_output, sub_profile)?)
        }
        "run" | "r" => CodeSubcommand::Run(parse_run_command(cursor, sub_output, sub_profile)?),
        "test" | "t" => CodeSubcommand::Test(parse_test_command(cursor, sub_output, sub_profile)?),
        "check" | "c" | "verify" => {
            CodeSubcommand::Check(parse_check_command(cursor, sub_output, sub_profile)?)
        }
        "emit" | "e" | "gen" => CodeSubcommand::Emit(parse_emit_command(cursor)?),
        "explain" => CodeSubcommand::Explain(parse_explain_command(cursor)?),
        _ => {
            return Err(ParseError::invalid_subcommand(format!(
                "unknown code subcommand: {sub}"
            )))
        }
    };

    Ok(FrontendCommand::Code(CodeCommand {
        output,
        profile: profile_args,
        command: subcommand,
    }))
}

fn parse_build_command(
    cursor: &mut ArgCursor,
    output: FrontendOutputArgs,
    profile: FrontendProfileArgs,
) -> Result<BuildCommand, ParseError> {
    let mut cmd = BuildCommand {
        output,
        profile,
        ..BuildCommand::default()
    };
    while let Some(token) = cursor.peek() {
        if token == "--" {
            break;
        }
        if token.starts_with("--") || token.starts_with("-D") {
            let token = cursor.advance().unwrap().to_string();
            parse_build_flag(
                &mut cmd.output,
                &mut cmd.profile,
                &mut cmd.roots,
                &mut cmd.options,
                &mut cmd.step,
                &mut cmd.locked,
                Some(&mut cmd.keep_build_dir),
                &token,
                cursor,
            )?;
        } else {
            // Positional = direct target
            if cmd.target.input.is_none() {
                cmd.target.input = Some(cursor.advance().unwrap().to_string());
            } else {
                break;
            }
        }
    }
    Ok(cmd)
}

fn parse_run_command(
    cursor: &mut ArgCursor,
    output: FrontendOutputArgs,
    profile: FrontendProfileArgs,
) -> Result<RunCommand, ParseError> {
    let mut cmd = RunCommand {
        output,
        profile,
        ..RunCommand::default()
    };
    let mut hit_separator = false;
    while let Some(token) = cursor.peek() {
        if token == "--" {
            cursor.advance();
            hit_separator = true;
            break;
        }
        if token.starts_with("--") || token.starts_with("-D") {
            let token = cursor.advance().unwrap().to_string();
            parse_build_flag(
                &mut cmd.output,
                &mut cmd.profile,
                &mut cmd.roots,
                &mut cmd.options,
                &mut cmd.step,
                &mut cmd.locked,
                Some(&mut cmd.keep_build_dir),
                &token,
                cursor,
            )?;
        } else if cmd.target.input.is_none() {
            cmd.target.input = Some(cursor.advance().unwrap().to_string());
        } else {
            // Additional positional before -- goes to args
            cmd.args.push(cursor.advance().unwrap().to_string());
        }
    }
    // Everything after -- goes to args
    if hit_separator {
        while let Some(token) = cursor.advance() {
            // After --, first token might become target.input if not set,
            // but clap's behavior with trailing_var_arg puts them all after the positional
            // Actually with clap, `fol code run -- --flag value` puts --flag into target.input
            // and value into args. Let's match that.
            if cmd.target.input.is_none() {
                cmd.target.input = Some(token.to_string());
            } else {
                cmd.args.push(token.to_string());
            }
        }
    }
    Ok(cmd)
}

fn parse_test_command(
    cursor: &mut ArgCursor,
    output: FrontendOutputArgs,
    profile: FrontendProfileArgs,
) -> Result<TestCommand, ParseError> {
    let mut cmd = TestCommand {
        output,
        profile,
        ..TestCommand::default()
    };
    while let Some(token) = cursor.peek() {
        if token.starts_with("--") || token.starts_with("-D") {
            let token = cursor.advance().unwrap().to_string();
            let (key, _) = split_eq(&token);
            match key {
                "--output" => {
                    cmd.output.output =
                        Some(parse_output_mode(&cursor.take_value(&token, "output")?)?)
                }
                "--profile" => {
                    cmd.profile.profile =
                        Some(parse_profile(&cursor.take_value(&token, "profile")?)?)
                }
                "--debug" => cmd.profile.debug = true,
                "--release" => cmd.profile.release = true,
                "--path" => cmd.path = Some(cursor.take_value(&token, "path")?),
                "--locked" => cmd.locked = true,
                "--step" => cmd.step.step = Some(cursor.take_value(&token, "step")?),
                "--target" => cmd.options.build_target = Some(cursor.take_value(&token, "target")?),
                "--optimize" => {
                    cmd.options.build_optimize = Some(cursor.take_value(&token, "optimize")?)
                }
                "--build-option" => cmd
                    .options
                    .build_options
                    .push(cursor.take_value(&token, "build-option")?),
                _ if token.starts_with("-D") => {
                    let val = if token.len() > 2 {
                        token[2..].to_string()
                    } else {
                        cursor.take_value(&token, "D")?
                    };
                    cmd.options.define.push(val);
                }
                "-h" | "--help" => {
                    return Err(ParseError::help(
                        "Usage: fol code test [OPTIONS]".to_string(),
                    ))
                }
                _ => return Err(ParseError::invalid(format!("unknown flag for test: {key}"))),
            }
        } else {
            break;
        }
    }
    Ok(cmd)
}

fn parse_check_command(
    cursor: &mut ArgCursor,
    output: FrontendOutputArgs,
    profile: FrontendProfileArgs,
) -> Result<CheckCommand, ParseError> {
    let mut cmd = CheckCommand {
        output,
        profile,
        ..CheckCommand::default()
    };
    while let Some(token) = cursor.peek() {
        if token.starts_with("--") || token.starts_with("-D") {
            let token = cursor.advance().unwrap().to_string();
            parse_build_flag(
                &mut cmd.output,
                &mut cmd.profile,
                &mut cmd.roots,
                &mut cmd.options,
                &mut cmd.step,
                &mut cmd.locked,
                None,
                &token,
                cursor,
            )?;
        } else if cmd.target.input.is_none() {
            cmd.target.input = Some(cursor.advance().unwrap().to_string());
        } else {
            break;
        }
    }
    Ok(cmd)
}

fn parse_emit_command(cursor: &mut ArgCursor) -> Result<EmitCommand, ParseError> {
    if matches!(cursor.peek(), Some("--help" | "-h")) {
        return Err(ParseError::help(emit_help()));
    }

    let sub = cursor
        .advance()
        .ok_or_else(|| ParseError::help(emit_help()))?;
    let subcommand = match sub {
        "rust" => EmitSubcommand::Rust(parse_emit_rust_command(cursor)?),
        "lowered" => EmitSubcommand::Lowered(parse_emit_lowered_command(cursor)?),
        _ => {
            return Err(ParseError::invalid_subcommand(format!(
                "unknown emit subcommand: {sub}"
            )))
        }
    };
    Ok(EmitCommand {
        command: subcommand,
    })
}

fn parse_emit_rust_command(cursor: &mut ArgCursor) -> Result<EmitRustCommand, ParseError> {
    let mut cmd = EmitRustCommand::default();
    while let Some(token) = cursor.peek() {
        if token.starts_with("--") {
            let token = cursor.advance().unwrap().to_string();
            let (key, _) = split_eq(&token);
            match key {
                "--output" => {
                    cmd.output.output =
                        Some(parse_output_mode(&cursor.take_value(&token, "output")?)?)
                }
                "--profile" => {
                    cmd.profile.profile =
                        Some(parse_profile(&cursor.take_value(&token, "profile")?)?)
                }
                "--debug" => cmd.profile.debug = true,
                "--release" => cmd.profile.release = true,
                "--std-root" => cmd.roots.std_root = Some(cursor.take_value(&token, "std-root")?),
                "--package-store-root" => {
                    cmd.roots.package_store_root =
                        Some(cursor.take_value(&token, "package-store-root")?)
                }
                "--keep-build-dir" => cmd.keep_build_dir = true,
                "-h" | "--help" => {
                    return Err(ParseError::help(
                        "Usage: fol code emit rust [OPTIONS] [PATH]".to_string(),
                    ))
                }
                _ => {
                    return Err(ParseError::invalid(format!(
                        "unknown flag for emit rust: {key}"
                    )))
                }
            }
        } else if cmd.target.input.is_none() {
            cmd.target.input = Some(cursor.advance().unwrap().to_string());
        } else {
            break;
        }
    }
    Ok(cmd)
}

fn parse_emit_lowered_command(cursor: &mut ArgCursor) -> Result<EmitLoweredCommand, ParseError> {
    let mut cmd = EmitLoweredCommand::default();
    while let Some(token) = cursor.peek() {
        if token.starts_with("--") {
            let token = cursor.advance().unwrap().to_string();
            let (key, _) = split_eq(&token);
            match key {
                "--output" => {
                    cmd.output.output =
                        Some(parse_output_mode(&cursor.take_value(&token, "output")?)?)
                }
                "--profile" => {
                    cmd.profile.profile =
                        Some(parse_profile(&cursor.take_value(&token, "profile")?)?)
                }
                "--debug" => cmd.profile.debug = true,
                "--release" => cmd.profile.release = true,
                "--std-root" => cmd.roots.std_root = Some(cursor.take_value(&token, "std-root")?),
                "--package-store-root" => {
                    cmd.roots.package_store_root =
                        Some(cursor.take_value(&token, "package-store-root")?)
                }
                "-h" | "--help" => {
                    return Err(ParseError::help(
                        "Usage: fol code emit lowered [OPTIONS] [PATH]".to_string(),
                    ))
                }
                _ => {
                    return Err(ParseError::invalid(format!(
                        "unknown flag for emit lowered: {key}"
                    )))
                }
            }
        } else if cmd.target.input.is_none() {
            cmd.target.input = Some(cursor.advance().unwrap().to_string());
        } else {
            break;
        }
    }
    Ok(cmd)
}

// ---------------------------------------------------------------------------
// tool
// ---------------------------------------------------------------------------

fn parse_tool_command(cursor: &mut ArgCursor) -> Result<FrontendCommand, ParseError> {
    let mut output = FrontendOutputArgs::default();

    while let Some(token) = cursor.peek() {
        if token == "--help" || token == "-h" {
            return Err(ParseError::help(tool_help()));
        }
        if token.starts_with("--") {
            let token = cursor.advance().unwrap().to_string();
            let (key, _) = split_eq(&token);
            match key {
                "--output" => {
                    output.output = Some(parse_output_mode(&cursor.take_value(&token, "output")?)?)
                }
                _ => return Err(ParseError::invalid(format!("unknown flag for tool: {key}"))),
            }
            continue;
        }
        break;
    }

    let sub = cursor
        .advance()
        .ok_or_else(|| ParseError::help(tool_help()))?;
    let subcommand = match sub {
        "lsp" => ToolSubcommand::Lsp(UnitCommand),
        "format" => ToolSubcommand::Format(parse_editor_path_command(cursor)?),
        "parse" => ToolSubcommand::Parse(parse_editor_path_command(cursor)?),
        "highlight" => ToolSubcommand::Highlight(parse_editor_path_command(cursor)?),
        "symbols" => ToolSubcommand::Symbols(parse_editor_path_command(cursor)?),
        "references" => ToolSubcommand::References(parse_editor_reference_command(cursor)?),
        "rename" => ToolSubcommand::Rename(parse_editor_rename_command(cursor)?),
        "complete" => ToolSubcommand::Complete(parse_editor_completion_command(cursor)?),
        "semantic-tokens" => ToolSubcommand::SemanticTokens(parse_editor_path_command(cursor)?),
        "tree" => ToolSubcommand::Tree(parse_tree_command(cursor)?),
        "clean" | "cl" | "purge" => ToolSubcommand::Clean(UnitCommand),
        "completion" | "completions" | "comp" => {
            ToolSubcommand::Completion(parse_completion_command(cursor)?)
        }
        _ => {
            return Err(ParseError::invalid_subcommand(format!(
                "unknown tool subcommand: {sub}"
            )))
        }
    };

    Ok(FrontendCommand::Tool(ToolCommand {
        output,
        command: subcommand,
    }))
}

fn parse_editor_path_command(cursor: &mut ArgCursor) -> Result<EditorPathCommand, ParseError> {
    let path = cursor
        .advance()
        .ok_or_else(|| ParseError::missing("expected a file path"))?
        .to_string();
    Ok(EditorPathCommand { path })
}

fn parse_editor_reference_command(
    cursor: &mut ArgCursor,
) -> Result<EditorReferenceCommand, ParseError> {
    let mut path: Option<String> = None;
    let mut line: Option<u32> = None;
    let mut character: Option<u32> = None;
    let mut exclude_declaration = false;

    while let Some(token) = cursor.peek() {
        if token.starts_with("--") {
            let token = cursor.advance().unwrap().to_string();
            let (key, _) = split_eq(&token);
            match key {
                "--line" => line = Some(parse_u32(&cursor.take_value(&token, "line")?, "line")?),
                "--character" => character = Some(parse_u32(&cursor.take_value(&token, "character")?, "character")?),
                "--exclude-declaration" => exclude_declaration = true,
                "-h" | "--help" => return Err(ParseError::help("Usage: fol tool references <PATH> --line <N> --character <N> [--exclude-declaration]".to_string())),
                _ => return Err(ParseError::invalid(format!("unknown flag for references: {key}"))),
            }
        } else if path.is_none() {
            path = Some(cursor.advance().unwrap().to_string());
        } else {
            break;
        }
    }

    Ok(EditorReferenceCommand {
        path: path.ok_or_else(|| ParseError::missing("references requires a file path"))?,
        line: line.ok_or_else(|| ParseError::missing("references requires --line"))?,
        character: character
            .ok_or_else(|| ParseError::missing("references requires --character"))?,
        exclude_declaration,
    })
}

fn parse_editor_completion_command(
    cursor: &mut ArgCursor,
) -> Result<EditorCompletionCommand, ParseError> {
    let mut path: Option<String> = None;
    let mut line: Option<u32> = None;
    let mut character: Option<u32> = None;

    while let Some(token) = cursor.peek() {
        if token.starts_with("--") {
            let token = cursor.advance().unwrap().to_string();
            let (key, _) = split_eq(&token);
            match key {
                "--line" => line = Some(parse_u32(&cursor.take_value(&token, "line")?, "line")?),
                "--character" => {
                    character = Some(parse_u32(
                        &cursor.take_value(&token, "character")?,
                        "character",
                    )?)
                }
                _ => {
                    return Err(ParseError::invalid(format!(
                        "unknown flag for complete: {key}"
                    )))
                }
            }
        } else if path.is_none() {
            path = Some(cursor.advance().unwrap().to_string());
        } else {
            break;
        }
    }

    Ok(EditorCompletionCommand {
        path: path.ok_or_else(|| ParseError::missing("complete requires a file path"))?,
        line: line.ok_or_else(|| ParseError::missing("complete requires --line"))?,
        character: character.ok_or_else(|| ParseError::missing("complete requires --character"))?,
    })
}

fn parse_editor_rename_command(cursor: &mut ArgCursor) -> Result<EditorRenameCommand, ParseError> {
    let mut path: Option<String> = None;
    let mut line: Option<u32> = None;
    let mut character: Option<u32> = None;
    let mut new_name: Option<String> = None;

    while let Some(token) = cursor.peek() {
        if token.starts_with("--") {
            let token = cursor.advance().unwrap().to_string();
            let (key, _) = split_eq(&token);
            match key {
                "--line" => line = Some(parse_u32(&cursor.take_value(&token, "line")?, "line")?),
                "--character" => {
                    character = Some(parse_u32(
                        &cursor.take_value(&token, "character")?,
                        "character",
                    )?)
                }
                _ => {
                    return Err(ParseError::invalid(format!(
                        "unknown flag for rename: {key}"
                    )))
                }
            }
        } else if path.is_none() {
            path = Some(cursor.advance().unwrap().to_string());
        } else if new_name.is_none() {
            new_name = Some(cursor.advance().unwrap().to_string());
        } else {
            break;
        }
    }

    Ok(EditorRenameCommand {
        path: path.ok_or_else(|| ParseError::missing("rename requires a file path"))?,
        line: line.ok_or_else(|| ParseError::missing("rename requires --line"))?,
        character: character.ok_or_else(|| ParseError::missing("rename requires --character"))?,
        new_name: new_name.ok_or_else(|| ParseError::missing("rename requires a new name"))?,
    })
}

fn parse_tree_command(cursor: &mut ArgCursor) -> Result<TreeCommand, ParseError> {
    if matches!(cursor.peek(), Some("--help" | "-h")) {
        return Err(ParseError::help(tree_help()));
    }
    let sub = cursor
        .advance()
        .ok_or_else(|| ParseError::help(tree_help()))?;
    match sub {
        "generate" => {
            let path = cursor
                .advance()
                .ok_or_else(|| ParseError::missing("tree generate requires a path"))?
                .to_string();
            Ok(TreeCommand {
                command: TreeSubcommand::Generate(TreeGenerateCommand { path }),
            })
        }
        _ => Err(ParseError::invalid_subcommand(format!(
            "unknown tree subcommand: {sub}"
        ))),
    }
}

fn parse_explain_command(cursor: &mut ArgCursor) -> Result<ExplainCommand, ParseError> {
    let mut code: Option<String> = None;
    let mut output: Option<OutputMode> = None;

    while let Some(token) = cursor.peek() {
        if token == "-h" || token == "--help" {
            return Err(ParseError::help(explain_help()));
        }
        if token.starts_with("--") {
            let token = cursor.advance().unwrap().to_string();
            let (key, _) = split_eq(&token);
            match key {
                "--output" => {
                    output = Some(parse_output_mode(&cursor.take_value(&token, "output")?)?)
                }
                "--json" => output = Some(OutputMode::Json),
                _ => {
                    return Err(ParseError::invalid(format!(
                        "unknown flag for explain: {key}"
                    )))
                }
            }
        } else if code.is_none() {
            code = Some(cursor.advance().unwrap().to_string());
        } else {
            break;
        }
    }

    let code = code.ok_or_else(|| {
        ParseError::missing("explain requires a diagnostic code, e.g. `fol code explain T1003`")
    })?;
    Ok(ExplainCommand { code, output })
}

fn parse_completion_command(cursor: &mut ArgCursor) -> Result<CompletionCommand, ParseError> {
    let shell = cursor
        .advance()
        .ok_or_else(|| ParseError::missing("completion requires a shell (bash, zsh, fish)"))?;
    let shell = match shell {
        "bash" => CompletionShellArg::Bash,
        "zsh" => CompletionShellArg::Zsh,
        "fish" => CompletionShellArg::Fish,
        _ => {
            return Err(ParseError::invalid(format!(
                "unknown shell: {shell} (expected bash, zsh, fish)"
            )))
        }
    };
    Ok(CompletionCommand { shell })
}

// ---------------------------------------------------------------------------
// Shared build flag parser
// ---------------------------------------------------------------------------

// Shared build parsing exposes each command's mutable option group explicitly;
// a wrapper would obscure which groups a command intentionally supports.
#[allow(clippy::too_many_arguments)]
fn parse_build_flag(
    output: &mut FrontendOutputArgs,
    profile: &mut FrontendProfileArgs,
    roots: &mut CompileRootArgs,
    options: &mut BuildOptionArgs,
    step: &mut BuildStepArgs,
    locked: &mut bool,
    keep_build_dir: Option<&mut bool>,
    token: &str,
    cursor: &mut ArgCursor,
) -> Result<(), ParseError> {
    let (key, _) = split_eq(token);
    match key {
        "--output" => {
            output.output = Some(parse_output_mode(&cursor.take_value(token, "output")?)?)
        }
        "--profile" => {
            profile.profile = Some(parse_profile(&cursor.take_value(token, "profile")?)?)
        }
        "--debug" => profile.debug = true,
        "--release" => profile.release = true,
        "--std-root" => roots.std_root = Some(cursor.take_value(token, "std-root")?),
        "--package-store-root" => {
            roots.package_store_root = Some(cursor.take_value(token, "package-store-root")?)
        }
        "--locked" => *locked = true,
        "--keep-build-dir" => {
            if let Some(k) = keep_build_dir {
                *k = true;
            }
        }
        "--step" => step.step = Some(cursor.take_value(token, "step")?),
        "--target" => options.build_target = Some(cursor.take_value(token, "target")?),
        "--optimize" => options.build_optimize = Some(cursor.take_value(token, "optimize")?),
        "--build-option" => options
            .build_options
            .push(cursor.take_value(token, "build-option")?),
        _ if token.starts_with("-D") => {
            let val = if token.len() > 2 {
                token[2..].to_string()
            } else {
                cursor.take_value(token, "D")?
            };
            options.define.push(val);
        }
        "-h" | "--help" => {
            return Err(ParseError::help(
                "Usage: fol code <command> [OPTIONS]".to_string(),
            ))
        }
        _ => return Err(ParseError::invalid(format!("unknown flag: {key}"))),
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn split_eq(token: &str) -> (&str, Option<&str>) {
    if let Some(pos) = token.find('=') {
        (&token[..pos], Some(&token[pos + 1..]))
    } else {
        (token, None)
    }
}

fn parse_output_mode(value: &str) -> Result<OutputMode, ParseError> {
    match value {
        "human" => Ok(OutputMode::Human),
        "plain" => Ok(OutputMode::Plain),
        "json" => Ok(OutputMode::Json),
        _ => Err(ParseError::invalid(format!(
            "unknown output mode: {value} (expected human, plain, json)"
        ))),
    }
}

fn parse_profile(value: &str) -> Result<FrontendProfile, ParseError> {
    match value {
        "debug" => Ok(FrontendProfile::Debug),
        "release" => Ok(FrontendProfile::Release),
        _ => Err(ParseError::invalid(format!(
            "unknown profile: {value} (expected debug, release)"
        ))),
    }
}

fn parse_u32(value: &str, name: &str) -> Result<u32, ParseError> {
    value
        .parse::<u32>()
        .map_err(|_| ParseError::invalid(format!("--{name} must be a number, got: {value}")))
}

fn check_profile_conflicts(profile: &FrontendProfileArgs) -> Result<(), ParseError> {
    let count = [profile.debug, profile.release, profile.profile.is_some()]
        .iter()
        .filter(|&&v| v)
        .count();
    if count > 1 {
        return Err(ParseError::conflict(
            "--debug, --release, and --profile are mutually exclusive",
        ));
    }
    Ok(())
}

fn env_output_mode() -> OutputMode {
    match std::env::var("FOL_OUTPUT").ok().as_deref() {
        Some("plain") => OutputMode::Plain,
        Some("json") => OutputMode::Json,
        _ => OutputMode::Human,
    }
}

fn env_profile() -> Option<FrontendProfile> {
    match std::env::var("FOL_PROFILE").ok().as_deref() {
        Some("release") => Some(FrontendProfile::Release),
        Some("debug") => Some(FrontendProfile::Debug),
        _ => None,
    }
}

fn env_profile_args() -> FrontendProfileArgs {
    FrontendProfileArgs {
        profile: env_profile(),
        ..FrontendProfileArgs::default()
    }
}

// ---------------------------------------------------------------------------
// Help rendering (for compatibility with old FrontendCli::command())
// ---------------------------------------------------------------------------

impl FrontendCli {
    /// Returns the root help text. Used by dispatch for `fol` with no args.
    pub fn root_help_text() -> String {
        root_help()
    }
}
