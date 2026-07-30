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
pub use crate::builtins::{div_int, len, mod_int, pow, pow_float, FolLength};
pub use crate::containers::{
    index_array, index_seq, index_set, index_vec, lookup_map, render_array, render_map, render_seq,
    render_set, render_vec, slice_seq, slice_vec, FolArray,
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

    pub fn with<R>(&self, read: impl FnOnce(&T) -> R) -> R {
        let value = self.lock();
        read(&value)
    }

    pub fn with_mut<R>(&self, write: impl FnOnce(&mut T) -> R) -> R {
        let mut value = self.lock();
        write(&mut value)
    }
}

pub fn echo<T: FolEchoFormat>(value: T) -> T {
    println!("{}", value.fol_echo_format());
    value
}

/// Write a string to stdout without a trailing newline and flush it — the
/// frame-rendering primitive for terminal programs.
pub fn write(value: FolStr) -> FolStr {
    use std::io::Write as _;
    print!("{}", value.as_str());
    let _ = std::io::stdout().flush();
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

/// The substring at a byte offset and length, clamped to the string and
/// snapped outward to UTF-8 boundaries so it never panics.
pub fn str_sub(text: FolStr, start: crate::value::FolInt, len: crate::value::FolInt) -> FolStr {
    let bytes = text.as_str().as_bytes();
    let total = bytes.len();
    let from = start.clamp(0, total as i64) as usize;
    let until = (start.max(0) as usize)
        .saturating_add(len.max(0) as usize)
        .min(total);
    let mut from = from.min(until);
    let mut until = until;
    while from < total && !text.as_str().is_char_boundary(from) {
        from += 1;
    }
    while until > from && !text.as_str().is_char_boundary(until) {
        until -= 1;
    }
    FolStr::new(&text.as_str()[from..until])
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

fn term_size() -> (crate::value::FolInt, crate::value::FolInt) {
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

#[cfg(test)]
mod tests {
    use super::*;

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
