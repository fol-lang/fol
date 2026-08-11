//! Effective hosted runtime tier selected when a public `memo` artifact
//! declares the bundled internal `standard` dependency. This is not a third
//! public `fol_model`.

use crate::core::RuntimeTier;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{mpsc, Arc};
use std::sync::{Mutex, OnceLock};
use std::thread::JoinHandle;

pub use crate::abi::{check_recoverable, recoverable_succeeded, FolRecover};
pub use crate::aggregate::{
    render_echo, render_entry, render_entry_debug, render_record, render_record_debug,
    FolEchoFormat, FolEntry, FolNamedValue, FolRecord,
};
pub use crate::builtins::{
    atan2, bit_and, bit_or, bit_xor, chr_to_int, chr_to_str, clz, cos, ctz, div_int, exp, flt_abs,
    flt_ceil, flt_floor, flt_round, flt_to_int, hypot, int_to_chr, int_to_flt, is_inf, is_nan, len,
    ln, log10, mod_int, parse_flt, pop_count, pow, pow_float, rotl, rotr, shl, shr, sin, sqrt, tan,
    FolLength,
};
pub use crate::containers::{
    clear_map, clear_vec, contains_map, get_map, index_array, index_seq, index_set, index_vec,
    insert_map, insert_vec, keys_map, lookup_map, pop_vec, push_vec, remove_map, remove_vec,
    render_array, render_map, render_seq, render_set, render_vec, slice_seq, slice_vec,
    store_array, store_vec, truncate_vec, values_map, FolArray,
};
pub use crate::error::require;
pub use crate::memo::{FolMap, FolSeq, FolSet, FolStr, FolVec};
pub use crate::shell::{
    unwrap_error_shell, unwrap_error_shell_ref, unwrap_optional_shell, unwrap_optional_shell_ref,
    FolError, FolOption,
};
pub use crate::value::{impossible, FolBool, FolChar, FolFloat, FolInt, FolNever};
pub use crate::{crate_name, CRATE_NAME};

pub const HAS_HEAP: bool = true;
pub const HAS_OS: bool = true;
pub const TIER: RuntimeTier = RuntimeTier::new("std", HAS_HEAP, HAS_OS);

fn task_handles() -> &'static Mutex<Vec<JoinHandle<()>>> {
    static TASKS: OnceLock<Mutex<Vec<JoinHandle<()>>>> = OnceLock::new();
    TASKS.get_or_init(|| Mutex::new(Vec::new()))
}

pub fn spawn_task<F>(task: F)
where
    F: FnOnce() + Send + 'static,
{
    task_handles()
        .lock()
        .unwrap_or_else(|error| error.into_inner())
        .push(std::thread::spawn(task));
}

/// Spawn a detached task (`[spn, det]`): the join handle is dropped, so the task
/// is never registered for join and is not awaited at scope or process exit.
pub fn spawn_detached<F>(task: F)
where
    F: FnOnce() + Send + 'static,
{
    drop(std::thread::spawn(task));
}

pub fn join_all_tasks() {
    let mut first_panic = None;
    loop {
        let handles = {
            let mut tasks = task_handles()
                .lock()
                .unwrap_or_else(|error| error.into_inner());
            std::mem::take(&mut *tasks)
        };
        if handles.is_empty() {
            break;
        }
        for handle in handles {
            if let Err(payload) = handle.join() {
                if first_panic.is_none() {
                    first_panic = Some(payload);
                }
            }
        }
    }
    if let Some(payload) = first_panic {
        std::panic::resume_unwind(payload);
    }
}

#[derive(Debug, Default)]
pub struct FolTaskJoinGuard;

pub fn task_join_guard() -> FolTaskJoinGuard {
    FolTaskJoinGuard
}

impl Drop for FolTaskJoinGuard {
    fn drop(&mut self) {
        let already_panicking = std::thread::panicking();
        let joined = std::panic::catch_unwind(std::panic::AssertUnwindSafe(join_all_tasks));
        if !already_panicking {
            if let Err(payload) = joined {
                std::panic::resume_unwind(payload);
            }
        }
    }
}

#[derive(Debug)]
pub struct FolEventual<T> {
    receiver: Mutex<Option<mpsc::Receiver<T>>>,
}

impl<T> Default for FolEventual<T> {
    fn default() -> Self {
        let (sender, receiver) = mpsc::channel();
        drop(sender);
        Self {
            receiver: Mutex::new(Some(receiver)),
        }
    }
}

impl<T> FolEventual<T> {
    pub fn await_value(self) -> T {
        self.receiver
            .into_inner()
            .unwrap_or_else(|error| error.into_inner())
            .expect("eventual can only be awaited once")
            .recv()
            .expect("eventual producer ended without a value")
    }
}

pub fn spawn_eventual<T, F>(task: F) -> FolEventual<T>
where
    T: Send + 'static,
    F: FnOnce() -> T + Send + 'static,
{
    let (sender, receiver) = mpsc::channel();
    task_handles()
        .lock()
        .unwrap_or_else(|error| error.into_inner())
        .push(std::thread::spawn(move || {
            let value = task();
            let _ = sender.send(value);
        }));
    FolEventual {
        receiver: Mutex::new(Some(receiver)),
    }
}

#[derive(Debug)]
pub struct FolChannel<T> {
    sender: Mutex<Option<mpsc::Sender<T>>>,
    receiver: Mutex<Option<mpsc::Receiver<T>>>,
    receiver_closed: AtomicBool,
}

impl<T> Default for FolChannel<T> {
    fn default() -> Self {
        let (sender, receiver) = mpsc::channel();
        Self {
            sender: Mutex::new(Some(sender)),
            receiver: Mutex::new(Some(receiver)),
            receiver_closed: AtomicBool::new(false),
        }
    }
}

impl<T> FolChannel<T> {
    pub fn acquire_sender(&self) -> Option<FolSender<T>> {
        self.sender
            .lock()
            .unwrap_or_else(|error| error.into_inner())
            .as_ref()
            .cloned()
            .map(FolSender)
    }

    /// Transfer the channel's unique receiver as a first-class `chn[rx, T]`
    /// value (V3_MEM §8.2). Receivers are unique, so this takes the receiver
    /// out: the owning channel binding can no longer receive afterward.
    pub fn acquire_receiver(&self) -> Option<FolReceiver<T>> {
        self.receiver
            .lock()
            .unwrap_or_else(|error| error.into_inner())
            .take()
            .map(FolReceiver::new)
    }

    pub fn send(&self, value: T) -> Result<(), T> {
        let Some(sender) = self.acquire_sender() else {
            return Err(value);
        };
        sender.send(value)
    }

    fn close_local_sender(&self) {
        self.sender
            .lock()
            .unwrap_or_else(|error| error.into_inner())
            .take();
    }

    pub fn receive_optional(&self) -> FolOption<T> {
        self.close_local_sender();
        let guard = self
            .receiver
            .lock()
            .unwrap_or_else(|error| error.into_inner());
        // A receiver moved out as a `chn[rx, T]` value leaves the owning
        // binding unable to receive: report closure rather than blocking.
        let Some(receiver) = guard.as_ref() else {
            self.receiver_closed.store(true, Ordering::Release);
            return None.into();
        };
        receiver.recv().ok().into()
    }

    pub fn try_receive(&self) -> FolOption<T> {
        self.close_local_sender();
        let guard = self
            .receiver
            .lock()
            .unwrap_or_else(|error| error.into_inner());
        let Some(receiver) = guard.as_ref() else {
            self.receiver_closed.store(true, Ordering::Release);
            return None.into();
        };
        match receiver.try_recv() {
            Ok(value) => Some(value).into(),
            Err(mpsc::TryRecvError::Empty) => None.into(),
            Err(mpsc::TryRecvError::Disconnected) => {
                self.receiver_closed.store(true, Ordering::Release);
                None.into()
            }
        }
    }

    pub fn is_closed(&self) -> bool {
        self.receiver_closed.load(Ordering::Acquire)
    }
}

pub fn yield_processor() {
    std::thread::yield_now();
}

#[derive(Debug)]
pub struct FolSender<T>(mpsc::Sender<T>);

impl<T> Clone for FolSender<T> {
    fn clone(&self) -> Self {
        Self(self.0.clone())
    }
}

impl<T> Default for FolSender<T> {
    fn default() -> Self {
        let (sender, receiver) = mpsc::channel();
        drop(receiver);
        Self(sender)
    }
}

impl<T> FolSender<T> {
    pub fn send(&self, value: T) -> Result<(), T> {
        self.0.send(value).map_err(|error| error.0)
    }
}

/// A first-class `chn[rx, T]` receiver endpoint value (V3_MEM §8.2). Receivers
/// are unique: unlike `FolSender`, this handle is move-only and never `Clone`.
#[derive(Debug)]
pub struct FolReceiver<T> {
    receiver: mpsc::Receiver<T>,
    /// Latched once every sender is gone, mirroring `FolChannel`, so a `select`
    /// without a default arm can tell that a receiver will never yield again.
    closed: AtomicBool,
}

impl<T> Default for FolReceiver<T> {
    fn default() -> Self {
        let (sender, receiver) = mpsc::channel();
        drop(sender);
        Self {
            receiver,
            closed: AtomicBool::new(true),
        }
    }
}

impl<T> FolReceiver<T> {
    fn new(receiver: mpsc::Receiver<T>) -> Self {
        Self {
            receiver,
            closed: AtomicBool::new(false),
        }
    }

    pub fn receive_optional(&self) -> FolOption<T> {
        match self.receiver.recv() {
            Ok(value) => Some(value).into(),
            Err(_) => {
                self.closed.store(true, Ordering::Release);
                None.into()
            }
        }
    }

    pub fn try_receive(&self) -> FolOption<T> {
        match self.receiver.try_recv() {
            Ok(value) => Some(value).into(),
            Err(mpsc::TryRecvError::Empty) => None.into(),
            Err(mpsc::TryRecvError::Disconnected) => {
                self.closed.store(true, Ordering::Release);
                None.into()
            }
        }
    }

    pub fn is_closed(&self) -> bool {
        self.closed.load(Ordering::Acquire)
    }
}

#[derive(Debug)]
pub struct FolMutex<T> {
    value: Arc<Mutex<T>>,
}

impl<T: Default> Default for FolMutex<T> {
    fn default() -> Self {
        Self {
            value: Arc::new(Mutex::new(T::default())),
        }
    }
}

impl<T> Clone for FolMutex<T> {
    fn clone(&self) -> Self {
        Self {
            value: self.value.clone(),
        }
    }
}

impl<T> FolMutex<T> {
    pub fn from_value(value: T) -> Self {
        Self {
            value: Arc::new(Mutex::new(value)),
        }
    }

    pub fn lock(&self) -> std::sync::MutexGuard<'_, T> {
        self.value.lock().unwrap_or_else(|error| error.into_inner())
    }

    /// Put `value` under the existing lock rather than swapping the handle.
    /// A guard borrows the place its handle lives in, so assigning that place
    /// is what a routine holding a guard elsewhere in its block graph cannot
    /// do; replacing the guarded value leaves the place alone.
    pub fn replace_value(&self, value: T) {
        *self.lock() = value;
    }

    pub fn with<R>(&self, read: impl FnOnce(&T) -> R) -> R {
        let value = self.lock();
        read(&value)
    }

    pub fn with_mut<R>(&self, write: impl FnOnce(&mut T) -> R) -> R {
        let mut value = self.lock();
        write(&mut value)
    }
}

/// Write a line to stdout, treating a closed pipe as a normal end.
///
/// `println!` PANICS when the reader is gone, so `prog | head -2` used to end
/// with a Rust panic and exit 101 — a stack trace naming a std source file, for
/// something every Unix tool does silently. A broken pipe means the consumer
/// stopped listening, which is not this program's failure.
pub fn echo<T: FolEchoFormat>(value: T) -> T {
    use std::io::Write as _;
    let rendered = value.fol_echo_format();
    let mut out = std::io::stdout().lock();
    if writeln!(out, "{rendered}").is_err() {
        std::process::exit(0);
    }
    value
}

/// Write a string to stdout without a trailing newline and flush it — the
/// frame-rendering primitive for terminal programs.
pub fn write(value: FolStr) -> FolStr {
    use std::io::Write as _;
    let mut out = std::io::stdout().lock();
    if write!(out, "{}", value.as_str()).is_err() || out.flush().is_err() {
        std::process::exit(0);
    }
    value
}

/// The shared stdin byte feed: one reader thread owns stdin so blocking and
/// timed reads can coexist without competing for the handle.
/// One shared reader thread serves both key primitives, because a timed-out
/// read must not lose the byte that arrives late — it has to be buffered for
/// the next call.
///
/// The thread reads **only when a byte has been requested**. An eager reader
/// would sit in a blocking `read` forever and steal input from any child
/// process that inherits stdin, which is exactly what `shell()` — the TUI
/// suspend/exec primitive — hands its editor or pager.
struct KeyFeed {
    requests: std::sync::mpsc::Sender<()>,
    bytes: std::sync::Mutex<std::sync::mpsc::Receiver<crate::value::FolInt>>,
    /// Requested reads that have not been consumed yet. While this is zero the
    /// thread is parked and stdin belongs to whoever else wants it.
    outstanding: std::sync::atomic::AtomicUsize,
}

fn key_feed() -> &'static KeyFeed {
    static FEED: std::sync::OnceLock<KeyFeed> = std::sync::OnceLock::new();
    FEED.get_or_init(|| {
        let (request_sender, request_receiver) = std::sync::mpsc::channel::<()>();
        let (byte_sender, byte_receiver) = std::sync::mpsc::channel();
        std::thread::spawn(move || {
            use std::io::Read as _;
            let mut stdin = std::io::stdin();
            let mut buffer = [0u8; 1];
            // Park until a read is actually asked for.
            while request_receiver.recv().is_ok() {
                let value = match stdin.read(&mut buffer) {
                    Ok(1) => crate::value::FolInt::from(buffer[0]),
                    _ => -1,
                };
                if byte_sender.send(value).is_err() {
                    break;
                }
            }
        });
        KeyFeed {
            requests: request_sender,
            bytes: std::sync::Mutex::new(byte_receiver),
            outstanding: std::sync::atomic::AtomicUsize::new(0),
        }
    })
}

impl KeyFeed {
    /// Ask for a byte unless a previous request is still pending (a timed-out
    /// `read_key_ms` leaves one behind, and its byte lands in the channel).
    fn request(&self) {
        use std::sync::atomic::Ordering;
        if self.outstanding.load(Ordering::Acquire) == 0 {
            self.outstanding.fetch_add(1, Ordering::AcqRel);
            let _ = self.requests.send(());
        }
    }

    fn consumed(&self) {
        use std::sync::atomic::Ordering;
        let _ = self
            .outstanding
            .fetch_update(Ordering::AcqRel, Ordering::Acquire, |current| {
                Some(current.saturating_sub(1))
            });
    }

    fn idle(&self) -> bool {
        self.outstanding.load(std::sync::atomic::Ordering::Acquire) == 0
    }
}

/// Block for one byte of standard input. Yields -1 at end of input so callers
/// can end their read loop without a recoverable shell.
pub fn read_key() -> crate::value::FolInt {
    let feed = key_feed();
    let bytes = feed
        .bytes
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    feed.request();
    match bytes.recv() {
        Ok(byte) => {
            feed.consumed();
            byte
        }
        Err(_) => {
            feed.consumed();
            -1
        }
    }
}

/// One byte of standard input within a timeout: the byte value, -2 when the
/// timeout elapses first, or -1 at end of input. The escape-sequence
/// disambiguator for key decoders.
pub fn read_key_ms(timeout_ms: crate::value::FolInt) -> crate::value::FolInt {
    let feed = key_feed();
    let bytes = feed
        .bytes
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    feed.request();
    match bytes.recv_timeout(std::time::Duration::from_millis(timeout_ms.max(0) as u64)) {
        Ok(byte) => {
            feed.consumed();
            byte
        }
        // The request stays outstanding: its byte will arrive later and the
        // next call picks it up instead of asking for another one.
        Err(std::sync::mpsc::RecvTimeoutError::Timeout) => -2,
        Err(std::sync::mpsc::RecvTimeoutError::Disconnected) => {
            feed.consumed();
            -1
        }
    }
}

/// Whether the shared reader is parked, i.e. nothing is competing for stdin.
pub fn stdin_is_idle() -> bool {
    key_feed().idle()
}

/// The substring at a byte offset and length, clamped to the string and snapped
/// to UTF-8 boundaries.
///
/// Both ends move *forward* to a boundary: a start inside a character skips the
/// partial character rather than emitting half of it, and a length that stops
/// inside one takes the whole character. Moving them in opposite directions is
/// what used to let the start overtake the end and panic the program on a slice
/// like `sub("héllo", 2, 0)`, so the end is pinned to the start before it snaps.
pub fn str_sub(text: FolStr, start: crate::value::FolInt, len: crate::value::FolInt) -> FolStr {
    let source = text.as_str();
    let total = source.len();

    let mut from = start.clamp(0, total as i64) as usize;
    while from < total && !source.is_char_boundary(from) {
        from += 1;
    }

    let mut until = (start.max(0) as usize)
        .saturating_add(len.max(0) as usize)
        .min(total)
        .max(from);
    while until < total && !source.is_char_boundary(until) {
        until += 1;
    }

    FolStr::new(&source[from..until])
}

/// The byte value at an index, or -1 outside the string.
pub fn str_byte(text: FolStr, index: crate::value::FolInt) -> crate::value::FolInt {
    if index < 0 {
        return -1;
    }
    text.as_str()
        .as_bytes()
        .get(index as usize)
        .map(|byte| *byte as crate::value::FolInt)
        .unwrap_or(-1)
}

/// A one-byte string from an ASCII byte value.
///
/// Empty for anything outside 0-127: a FOL `str` is UTF-8, so a lone byte in
/// 128-255 is not a valid one-byte string. Returning a replacement character
/// instead would hand back three bytes and quietly break `str_byte`
/// round-trips; callers reassembling multi-byte text must build the whole
/// sequence themselves.
pub fn byte_to_str(value: crate::value::FolInt) -> FolStr {
    if !(0..=127).contains(&value) {
        return FolStr::new("");
    }
    FolStr::new((value as u8 as char).to_string())
}

/// Enable or disable terminal raw mode via `stty` on the controlling
/// terminal; forwards the requested state (a no-op when stdin is not a tty or
/// `stty` is unavailable).
pub fn raw_mode(enable: bool) -> bool {
    let mut command = std::process::Command::new("stty");
    if enable {
        command.args(["raw", "-echo"]);
    } else {
        command.arg("sane");
    }
    let _ = command
        .stdin(std::process::Stdio::inherit())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status();
    enable
}

/// Sleep the current thread for the given milliseconds; negative values are
/// treated as zero. Forwards the requested duration.
pub fn sleep_ms(ms: crate::value::FolInt) -> crate::value::FolInt {
    std::thread::sleep(std::time::Duration::from_millis(ms.max(0) as u64));
    ms
}

/// Milliseconds since the unix epoch.
pub fn now_ms() -> crate::value::FolInt {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|elapsed| elapsed.as_millis() as crate::value::FolInt)
        .unwrap_or(0)
}

/// How long a probed terminal size is reused. A redraw loop asks for the size
/// every frame; spawning `stty` that often is pure overhead. The window is
/// short enough that a resize is still picked up promptly.
const TERM_SIZE_TTL: std::time::Duration = std::time::Duration::from_millis(200);

fn term_size() -> (crate::value::FolInt, crate::value::FolInt) {
    static CACHE: Mutex<
        Option<(
            (crate::value::FolInt, crate::value::FolInt),
            std::time::Instant,
        )>,
    > = Mutex::new(None);
    let mut cache = CACHE.lock().unwrap_or_else(|error| error.into_inner());
    if let Some((size, probed_at)) = *cache {
        if probed_at.elapsed() < TERM_SIZE_TTL {
            return size;
        }
    }
    let size = probe_term_size();
    *cache = Some((size, std::time::Instant::now()));
    size
}

fn probe_term_size() -> (crate::value::FolInt, crate::value::FolInt) {
    let probed = std::process::Command::new("stty")
        .arg("size")
        .stdin(std::process::Stdio::inherit())
        .output()
        .ok()
        .and_then(|output| {
            let text = String::from_utf8_lossy(&output.stdout).trim().to_string();
            let mut parts = text.split_whitespace();
            let rows = parts.next()?.parse::<i64>().ok()?;
            let cols = parts.next()?.parse::<i64>().ok()?;
            Some((rows, cols))
        });
    probed.unwrap_or((24, 80))
}

/// The terminal width in columns (80 when it cannot be determined).
pub fn term_cols() -> crate::value::FolInt {
    term_size().1
}

/// The terminal height in rows (24 when it cannot be determined).
pub fn term_rows() -> crate::value::FolInt {
    term_size().0
}

/// Render an integer as its decimal string.
pub fn int_to_str(value: crate::value::FolInt) -> FolStr {
    FolStr::new(value.to_string())
}

pub fn module_name() -> &'static str {
    "std"
}

pub fn tier_name() -> &'static str {
    TIER.name
}

pub fn base_core_tier() -> RuntimeTier {
    crate::core::capabilities()
}

pub fn base_memo_tier() -> RuntimeTier {
    crate::memo::capabilities()
}

pub fn capabilities() -> RuntimeTier {
    TIER
}

/// The value of an environment variable, or the empty string when unset.
pub fn env_var(name: FolStr) -> FolStr {
    std::env::var(name.as_str())
        .map(FolStr::new)
        .unwrap_or_else(|_| FolStr::new(""))
}

/// Run a shell command attached to the terminal and yield its exit code
/// (-1 when it cannot start). The TUI suspend/exec primitive.
pub fn shell(command: FolStr) -> crate::value::FolInt {
    // The child inherits stdin. The shared key reader only touches stdin while
    // a read is outstanding, so after a completed `read_key` the child gets the
    // terminal to itself. A pending timed-out `read_key_ms` is the one case
    // where a single byte can still be taken by the reader first.
    debug_assert!(
        stdin_is_idle(),
        "shell() ran while a key read was still outstanding; the child may lose one byte of input"
    );
    match std::process::Command::new("sh")
        .arg("-c")
        .arg(command.as_str())
        .status()
    {
        // Shell conventions, so the three outcomes stay distinguishable instead
        // of all collapsing onto -1: the command's own status, 128+N when a
        // signal killed it, and 127 when the shell could not be launched.
        Ok(status) => match status.code() {
            Some(code) => code as crate::value::FolInt,
            None => {
                use std::os::unix::process::ExitStatusExt as _;
                status
                    .signal()
                    .map(|signal| 128 + signal as crate::value::FolInt)
                    .unwrap_or(-1)
            }
        },
        Err(_) => 127,
    }
}

/// Sorted directory entries joined by newlines, directories suffixed with a
/// slash; empty when the path cannot be read.
pub fn dir_list(path: FolStr) -> FolStr {
    let mut entries: Vec<String> = std::fs::read_dir(path.as_str())
        .map(|reader| {
            reader
                .filter_map(|entry| entry.ok())
                .map(|entry| {
                    let mut name = entry.file_name().to_string_lossy().to_string();
                    if entry.file_type().map(|kind| kind.is_dir()).unwrap_or(false) {
                        name.push('/');
                    }
                    name
                })
                .collect()
        })
        .unwrap_or_default();
    entries.sort();
    FolStr::new(entries.join("\n"))
}

/// The text contents of a file, or the empty string when unreadable.
pub fn read_file(path: FolStr) -> FolStr {
    FolStr::new(std::fs::read_to_string(path.as_str()).unwrap_or_default())
}

/// Writes text to a path: 0 on success, -1 when the write fails.
pub fn write_file(path: FolStr, contents: FolStr) -> crate::value::FolInt {
    match std::fs::write(path.as_str(), contents.as_str()) {
        Ok(()) => 0,
        Err(_) => -1,
    }
}

/// How many command-line arguments the program received, excluding the name it
/// was invoked as.
pub fn arg_count() -> crate::value::FolInt {
    (std::env::args_os().count().saturating_sub(1)) as crate::value::FolInt
}

/// The command-line argument at an index, or the empty string when the index is
/// out of range. Index 0 is the first argument after the program name.
pub fn arg_at(index: crate::value::FolInt) -> FolStr {
    if index < 0 {
        return FolStr::new(String::new());
    }
    let argument = std::env::args_os()
        .nth(index as usize + 1)
        .map(|value| value.to_string_lossy().into_owned())
        .unwrap_or_default();
    FolStr::new(argument)
}

/// Text written to standard error without a trailing newline, forwarded
/// unchanged so it can be chained.
pub fn write_err(value: FolStr) -> FolStr {
    use std::io::Write as _;
    let mut stream = std::io::stderr();
    let _ = stream.write_all(value.as_str().as_bytes());
    let _ = stream.flush();
    value
}

/// The byte index where a needle first occurs, or -1 when it does not. An empty
/// needle matches at the start, which is what `find` means everywhere else.
pub fn str_find(haystack: FolStr, needle: FolStr) -> crate::value::FolInt {
    match haystack.as_str().find(needle.as_str()) {
        Some(index) => index as crate::value::FolInt,
        None => -1,
    }
}

/// Every occurrence of one substring replaced by another. An empty needle is
/// returned unchanged rather than splicing the replacement between every byte.
pub fn str_replace(text: FolStr, from: FolStr, to: FolStr) -> FolStr {
    if from.as_str().is_empty() {
        return text;
    }
    FolStr::new(text.as_str().replace(from.as_str(), to.as_str()))
}

/// A string parsed as an integer, or the caller's fallback.
///
/// The fallback is an argument rather than a fixed sentinel because every
/// sentinel is also a legitimate parse result: -1 cannot mean both "the text
/// said -1" and "the text was not a number".
/// One line of standard input, newline stripped. End of input is an empty
/// string rather than a fault, so a read loop terminates on emptiness.
pub fn read_line() -> FolStr {
    let mut line = String::new();
    match std::io::BufRead::read_line(&mut std::io::stdin().lock(), &mut line) {
        Ok(0) | Err(_) => FolStr::new(String::new()),
        Ok(_) => {
            while line.ends_with('\n') || line.ends_with('\r') {
                line.pop();
            }
            FolStr::new(line)
        }
    }
}

pub fn read_all() -> FolStr {
    let mut buffer = String::new();
    match std::io::Read::read_to_string(&mut std::io::stdin().lock(), &mut buffer) {
        Ok(_) => FolStr::new(buffer),
        Err(_) => FolStr::new(String::new()),
    }
}

pub fn file_exists(path: FolStr) -> crate::value::FolBool {
    std::path::Path::new(path.as_str()).exists()
}

pub fn is_file(path: FolStr) -> crate::value::FolBool {
    std::path::Path::new(path.as_str()).is_file()
}

pub fn is_dir(path: FolStr) -> crate::value::FolBool {
    std::path::Path::new(path.as_str()).is_dir()
}

/// Milliseconds since the epoch, or -1 when the file or its metadata cannot be
/// read. A sentinel rather than a fault: staleness checks ask about files that
/// legitimately may not exist yet.
pub fn file_mtime(path: FolStr) -> crate::value::FolInt {
    std::fs::metadata(path.as_str())
        .and_then(|meta| meta.modified())
        .ok()
        .and_then(|time| time.duration_since(std::time::UNIX_EPOCH).ok())
        .map_or(-1, |elapsed| elapsed.as_millis() as crate::value::FolInt)
}

pub fn file_size(path: FolStr) -> crate::value::FolInt {
    std::fs::metadata(path.as_str()).map_or(-1, |meta| meta.len() as crate::value::FolInt)
}

/// The mutating filesystem hooks all report 0 for success and 1 for failure,
/// matching `write_file`, which already reports that way.
pub fn make_dir(path: FolStr) -> crate::value::FolInt {
    std::fs::create_dir_all(path.as_str()).map_or(1, |()| 0)
}

pub fn remove_file(path: FolStr) -> crate::value::FolInt {
    std::fs::remove_file(path.as_str()).map_or(1, |()| 0)
}

pub fn rename_file(from: FolStr, to: FolStr) -> crate::value::FolInt {
    std::fs::rename(from.as_str(), to.as_str()).map_or(1, |()| 0)
}

pub fn copy_file(from: FolStr, to: FolStr) -> crate::value::FolInt {
    std::fs::copy(from.as_str(), to.as_str()).map_or(1, |_| 0)
}

pub fn append_file(path: FolStr, contents: FolStr) -> crate::value::FolInt {
    use std::io::Write;
    std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(path.as_str())
        .and_then(|mut file| file.write_all(contents.as_str().as_bytes()))
        .map_or(1, |()| 0)
}

pub fn current_dir() -> FolStr {
    std::env::current_dir().map_or_else(
        |_| FolStr::new(String::new()),
        |path| FolStr::new(path.display().to_string()),
    )
}

pub fn exit_process(status: crate::value::FolInt) -> crate::value::FolNever {
    std::process::exit(status as i32);
}

/// A command's standard output. `shell` reports only the exit status, which is
/// why capturing needed its own hook rather than a flag.
pub fn shell_out(command: FolStr) -> FolStr {
    std::process::Command::new("sh")
        .arg("-c")
        .arg(command.as_str())
        .output()
        .map_or_else(
            |_| FolStr::new(String::new()),
            |output| FolStr::new(String::from_utf8_lossy(&output.stdout).into_owned()),
        )
}

pub fn parse_int(text: FolStr, fallback: crate::value::FolInt) -> crate::value::FolInt {
    text.as_str()
        .trim()
        .parse::<crate::value::FolInt>()
        .unwrap_or(fallback)
}

/// A float rendered with a fixed number of decimal places, clamped to what
/// f64 can actually distinguish.
pub fn float_to_str(value: crate::value::FolFloat, decimals: crate::value::FolInt) -> FolStr {
    let places = decimals.clamp(0, 17) as usize;
    FolStr::new(format!("{value:.places$}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn str_sub_never_panics_on_a_start_inside_a_character() {
        // Every interior offset of a 2-byte and a 4-byte character, at the
        // lengths that used to invert the range.
        for text in ["héllo", "a😀b"] {
            for start in 0..=text.len() {
                for len in 0..3 {
                    let taken = str_sub(FolStr::new(text), start as i64, len as i64);
                    assert!(
                        text.contains(taken.as_str()),
                        "sub({text:?}, {start}, {len}) should be a slice of the input"
                    );
                }
            }
        }
    }

    #[test]
    fn str_sub_snaps_both_ends_forward_to_character_boundaries() {
        // A zero length stays empty instead of widening to a whole character.
        assert_eq!(str_sub(FolStr::new("héllo"), 2, 0).as_str(), "");
        // find() reports a byte index, so find-then-sub has to return the
        // character that find located.
        assert_eq!(str_sub(FolStr::new("unié"), 3, 1).as_str(), "é");
        assert_eq!(str_sub(FolStr::new("hello"), 1, 3).as_str(), "ell");
        // Out-of-range ends clamp rather than panicking.
        assert_eq!(str_sub(FolStr::new("hi"), 0, 99).as_str(), "hi");
        assert_eq!(str_sub(FolStr::new("hi"), 99, 1).as_str(), "");
        assert_eq!(str_sub(FolStr::new("hi"), -5, 1).as_str(), "h");
    }

    fn task_registry_test_guard() -> std::sync::MutexGuard<'static, ()> {
        static TEST_TASKS: std::sync::Mutex<()> = std::sync::Mutex::new(());
        TEST_TASKS.lock().unwrap_or_else(|error| error.into_inner())
    }

    #[derive(Debug, Clone, PartialEq, Eq)]
    struct DemoEcho(&'static str);

    impl FolEchoFormat for DemoEcho {
        fn fol_echo_format(&self) -> String {
            format!("demo({})", self.0)
        }
    }

    #[test]
    fn std_tier_marks_heap_and_os() {
        assert_eq!(module_name(), "std");
        assert_eq!(tier_name(), "std");
        assert_eq!(TIER, RuntimeTier::new("std", true, true));
        assert_eq!(capabilities(), TIER);
    }

    #[test]
    fn std_tier_builds_on_core_and_memo_tiers() {
        assert_eq!(base_core_tier(), crate::core::TIER);
        assert_eq!(base_memo_tier(), crate::memo::TIER);
        assert!(base_memo_tier().has_heap);
        assert!(capabilities().has_heap);
        assert!(capabilities().has_os);
    }

    #[test]
    fn spawned_tasks_and_nested_spawns_are_joined() {
        let _task_registry = task_registry_test_guard();
        let completed = std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(0));
        let outer_completed = completed.clone();
        spawn_task(move || {
            outer_completed.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
            let inner_completed = outer_completed.clone();
            spawn_task(move || {
                inner_completed.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
            });
        });

        join_all_tasks();

        assert_eq!(completed.load(std::sync::atomic::Ordering::SeqCst), 2);
    }

    #[test]
    fn cloned_senders_feed_one_uncloned_channel_receiver() {
        let _task_registry = task_registry_test_guard();
        let channel = FolChannel::default();
        let first = channel
            .acquire_sender()
            .expect("sender acquired before receiver use");
        let second = first.clone();
        spawn_task(move || first.send(19).expect("receiver remains open"));
        spawn_task(move || second.send(23).expect("receiver remains open"));

        let left = channel
            .receive_optional()
            .into_option()
            .expect("first payload present");
        let right = channel
            .receive_optional()
            .into_option()
            .expect("second payload present");
        join_all_tasks();

        assert_eq!(left + right, 42);
    }

    #[test]
    fn receiver_acquisition_relinquishes_only_the_local_transmitter() {
        let channel = FolChannel::default();
        let sender = channel
            .acquire_sender()
            .expect("sender acquired before receiver use");
        sender.send(19).expect("receiver remains open");

        assert_eq!(channel.receive_optional().into_option(), Some(19));
        assert!(channel.acquire_sender().is_none());

        sender.send(23).expect("pre-acquired sender remains valid");
        assert_eq!(channel.receive_optional().into_option(), Some(23));
    }

    #[test]
    fn awaiting_an_eventual_consumes_its_runtime_handle() {
        let _task_registry = task_registry_test_guard();
        let eventual = spawn_eventual(|| 42);
        assert_eq!(eventual.await_value(), 42);
        join_all_tasks();
    }

    #[test]
    fn task_join_guard_joins_during_unwind() {
        let _task_registry = task_registry_test_guard();
        let completed = Arc::new(AtomicBool::new(false));
        let task_completed = completed.clone();
        let outcome = std::panic::catch_unwind(move || {
            let _guard = task_join_guard();
            spawn_task(move || task_completed.store(true, Ordering::Release));
            panic!("exercise generated-entry unwind");
        });

        assert!(outcome.is_err());
        assert!(completed.load(Ordering::Acquire));
    }

    #[test]
    fn task_join_drains_remaining_handles_before_rethrowing_a_panic() {
        let _task_registry = task_registry_test_guard();
        let completed = Arc::new(AtomicBool::new(false));
        let task_completed = completed.clone();
        spawn_task(|| panic!("first task fails"));
        spawn_task(move || {
            std::thread::sleep(std::time::Duration::from_millis(100));
            task_completed.store(true, Ordering::Release);
        });

        let outcome = std::panic::catch_unwind(join_all_tasks);

        assert!(outcome.is_err());
        assert!(completed.load(Ordering::Acquire));
    }

    #[test]
    fn task_join_guard_does_not_double_panic_during_entry_unwind() {
        let _task_registry = task_registry_test_guard();
        let outcome = std::panic::catch_unwind(|| {
            let _guard = task_join_guard();
            spawn_task(|| panic!("task fails during entry unwind"));
            panic!("entry fails");
        });

        assert!(outcome.is_err());
    }

    #[test]
    fn explicit_mutex_lock_blocks_other_handles_until_unlock() {
        let owner = FolMutex::from_value(1i64);
        let contender = owner.clone();
        let (started_tx, started_rx) = mpsc::channel();
        let (entered_tx, entered_rx) = mpsc::channel();

        let mut guard = owner.lock();
        let handle = std::thread::spawn(move || {
            started_tx.send(()).expect("announce mutex access");
            contender.with_mut(|value| {
                *value += 1;
                entered_tx.send(()).expect("announce protected access");
            });
        });

        started_rx.recv().expect("contender started");
        assert!(entered_rx
            .recv_timeout(std::time::Duration::from_millis(100))
            .is_err());
        *guard += 40;
        drop(guard);

        entered_rx.recv().expect("contender entered after unlock");
        handle.join().expect("contender finished");
        assert_eq!(owner.with(|value| *value), 42);
    }

    #[test]
    fn runtime_echo_trait_and_helpers_freeze_backend_hook_boundary() {
        let value = DemoEcho("trace");

        assert_eq!(render_echo(&value), "demo(trace)");
        assert_eq!(echo(value.clone()), value);
    }

    #[test]
    fn runtime_echo_formats_builtin_scalars_and_strings() {
        let text = FolStr::from("Ada");

        assert_eq!(render_echo(&7i64), "7");
        assert_eq!(render_echo(&3.5f64), "3.5");
        assert_eq!(render_echo(&true), "true");
        assert_eq!(render_echo(&'x'), "x");
        assert_eq!(render_echo(&text), "Ada");
        assert_eq!(echo(text.clone()), text);
    }

    #[test]
    fn runtime_echo_formats_current_v1_container_families() {
        let array: FolArray<i64, 3> = [1, 2, 3];
        let vector = FolVec::from_items(vec![1, 2, 3]);
        let sequence = FolSeq::from_items(vec![1, 2, 3]);
        let set = FolSet::from_items(vec![3, 1, 2]);
        let map = FolMap::from_pairs(vec![(FolStr::from("lin"), 2), (FolStr::from("ada"), 1)]);

        assert_eq!(render_echo(&array), "arr[1, 2, 3]");
        assert_eq!(render_echo(&vector), "vec[1, 2, 3]");
        assert_eq!(render_echo(&sequence), "seq[1, 2, 3]");
        assert_eq!(render_echo(&set), "set{1, 2, 3}");
        assert_eq!(render_echo(&map), "map{ada: 1, lin: 2}");
    }

    #[test]
    fn runtime_echo_formats_current_v1_shell_families() {
        let some = FolOption::some(FolStr::from("Ada"));
        let nil = FolOption::<FolStr>::nil();
        let error = FolError::new(FolStr::from("broken"));

        assert_eq!(render_echo(&some), "some(Ada)");
        assert_eq!(render_echo(&nil), "nil");
        assert_eq!(render_echo(&error), "err(broken)");
    }

    #[test]
    fn runtime_echo_formats_nested_v1_values_stably() {
        let nested_seq =
            FolSeq::from_items(vec![FolOption::some(FolStr::from("Ada")), FolOption::nil()]);
        let nested_map = FolMap::from_pairs(vec![
            (
                FolStr::from("left"),
                FolError::new(FolSeq::from_items(vec![1i64, 2, 3])),
            ),
            (
                FolStr::from("right"),
                FolError::new(FolSeq::from_items(vec![4i64, 5])),
            ),
        ]);

        assert_eq!(render_echo(&nested_seq), "seq[some(Ada), nil]");
        assert_eq!(
            render_echo(&nested_map),
            "map{left: err(seq[1, 2, 3]), right: err(seq[4, 5])}"
        );
    }

    #[test]
    fn runtime_echo_formats_nested_container_values_stably() {
        let nested_seq = FolSeq::from_items(vec![
            FolSeq::from_items(vec![1i64, 2]),
            FolSeq::from_items(vec![3i64]),
        ]);
        let nested_map = FolMap::from_pairs(vec![
            (FolStr::from("left"), FolSet::from_items(vec![3i64, 1, 2])),
            (FolStr::from("right"), FolSet::from_items(vec![5i64, 4])),
        ]);

        assert_eq!(render_echo(&nested_seq), "seq[seq[1, 2], seq[3]]");
        assert_eq!(
            render_echo(&nested_map),
            "map{left: set{1, 2, 3}, right: set{4, 5}}"
        );
    }
}
