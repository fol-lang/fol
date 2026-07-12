//! Minimal `colored`-style ANSI trait for strings.
//!
//! Colors are automatically disabled when stdout is not a terminal.
//! Call [`set_enabled`] to override.
//!
//! ```ignore
//! use crate::ansi::Colored;
//! let s = "hello".red().bold();
//! let s = "path".cyan();
//! ```

use std::io::IsTerminal;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Once;

static ENABLED: AtomicBool = AtomicBool::new(false);
static INIT: Once = Once::new();

fn init() {
    INIT.call_once(|| {
        ENABLED.store(std::io::stdout().is_terminal(), Ordering::Relaxed);
    });
}

pub fn set_enabled(on: bool) {
    init();
    ENABLED.store(on, Ordering::Relaxed);
}

pub fn enabled() -> bool {
    init();
    ENABLED.load(Ordering::Relaxed)
}

// ── SGR codes ───────────────────────────────────────────────────────

const RESET: &str = "\x1b[0m";

const BOLD: &str = "1";
const DIM: &str = "2";
const ITALIC: &str = "3";

const FG_BLACK: &str = "30";
const FG_RED: &str = "31";
const FG_GREEN: &str = "32";
const FG_YELLOW: &str = "33";
const FG_BLUE: &str = "34";
const FG_CYAN: &str = "36";

const FG_BRIGHT_BLACK: &str = "90";
const FG_BRIGHT_BLUE: &str = "94";

const BG_RED: &str = "41";
const BG_YELLOW: &str = "43";
const BG_BLUE: &str = "44";

// ── Styled wrapper ──────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub struct Styled {
    text: String,
    codes: Vec<&'static str>,
}

impl Styled {
    fn new(text: String) -> Self {
        Self { text, codes: Vec::new() }
    }

    fn with(mut self, code: &'static str) -> Self {
        self.codes.push(code);
        self
    }

    // modifiers
    pub fn bold(self) -> Self { self.with(BOLD) }
    pub fn dim(self) -> Self { self.with(DIM) }
    pub fn italic(self) -> Self { self.with(ITALIC) }

    // foreground colors
    pub fn black(self) -> Self { self.with(FG_BLACK) }
    pub fn red(self) -> Self { self.with(FG_RED) }
    pub fn green(self) -> Self { self.with(FG_GREEN) }
    pub fn yellow(self) -> Self { self.with(FG_YELLOW) }
    pub fn blue(self) -> Self { self.with(FG_BLUE) }
    pub fn cyan(self) -> Self { self.with(FG_CYAN) }

    // bright foreground
    pub fn bright_black(self) -> Self { self.with(FG_BRIGHT_BLACK) }
    pub fn bright_blue(self) -> Self { self.with(FG_BRIGHT_BLUE) }

    // background colors (for chips/badges)
    pub fn on_red(self) -> Self { self.with(BG_RED) }
    pub fn on_yellow(self) -> Self { self.with(BG_YELLOW) }
    pub fn on_blue(self) -> Self { self.with(BG_BLUE) }
}

impl std::fmt::Display for Styled {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if !enabled() || self.codes.is_empty() {
            return f.write_str(&self.text);
        }
        f.write_str("\x1b[")?;
        for (i, code) in self.codes.iter().enumerate() {
            if i > 0 {
                f.write_str(";")?;
            }
            f.write_str(code)?;
        }
        f.write_str("m")?;
        f.write_str(&self.text)?;
        f.write_str(RESET)
    }
}

// ── Trait ────────────────────────────────────────────────────────────

pub trait Colored {
    fn styled(self) -> Styled;

    // convenience — one call gets you a Display-able Styled
    fn bold(self) -> Styled where Self: Sized { self.styled().bold() }
    fn dim(self) -> Styled where Self: Sized { self.styled().dim() }

    fn black(self) -> Styled where Self: Sized { self.styled().black() }
    fn red(self) -> Styled where Self: Sized { self.styled().red() }
    fn green(self) -> Styled where Self: Sized { self.styled().green() }
    fn yellow(self) -> Styled where Self: Sized { self.styled().yellow() }
    fn blue(self) -> Styled where Self: Sized { self.styled().blue() }
    fn cyan(self) -> Styled where Self: Sized { self.styled().cyan() }

    fn bright_black(self) -> Styled where Self: Sized { self.styled().bright_black() }
    fn bright_blue(self) -> Styled where Self: Sized { self.styled().bright_blue() }
}

impl Colored for &str {
    fn styled(self) -> Styled { Styled::new(self.to_string()) }
}

impl Colored for String {
    fn styled(self) -> Styled { Styled::new(self) }
}

impl Colored for &String {
    fn styled(self) -> Styled { Styled::new(self.clone()) }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Mutex;

    // The `ENABLED` flag these tests toggle is process-global, so they must not
    // run concurrently or they race and observe each other's state.
    static TEST_GUARD: Mutex<()> = Mutex::new(());

    fn guard() -> std::sync::MutexGuard<'static, ()> {
        TEST_GUARD.lock().unwrap_or_else(|poisoned| poisoned.into_inner())
    }

    #[test]
    fn trait_methods_emit_escape_sequences_when_enabled() {
        let _guard = guard();
        set_enabled(true);
        assert_eq!(format!("{}", "hi".cyan()), "\x1b[36mhi\x1b[0m");
        assert_eq!(format!("{}", "err".red().bold()), "\x1b[31;1merr\x1b[0m");
        assert_eq!(format!("{}", "ok".bold().green()), "\x1b[1;32mok\x1b[0m");
    }

    #[test]
    fn trait_methods_pass_through_when_disabled() {
        let _guard = guard();
        set_enabled(false);
        assert_eq!(format!("{}", "hi".cyan()), "hi");
        assert_eq!(format!("{}", "err".red().bold()), "err");
        set_enabled(true);
    }

    #[test]
    fn chained_modifiers_combine() {
        let _guard = guard();
        set_enabled(true);
        let s = format!("{}", "warn".bold().yellow().dim());
        assert!(s.starts_with("\x1b[1;33;2m"));
        assert!(s.ends_with("\x1b[0m"));
        assert!(s.contains("warn"));
    }

    #[test]
    fn string_and_str_ref_both_work() {
        let _guard = guard();
        set_enabled(true);
        let owned = String::from("owned");
        let from_owned = format!("{}", owned.red());
        let from_ref = format!("{}", "ref".red());
        assert!(from_owned.contains("owned"));
        assert!(from_ref.contains("ref"));
    }
}
