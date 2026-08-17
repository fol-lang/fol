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
    abs, acos, asin, atan, atan2, bit_and, bit_not, bit_or, bit_xor, checked_add, checked_div,
    checked_mul, checked_sub, chr_to_int, chr_to_str, clz, cos, ctz, div_int, exp, flt_abs,
    flt_bits, flt_ceil, flt_copysign, flt_floor, flt_from_bits, flt_is_finite, flt_mul_add,
    flt_next_after, flt_rem, flt_round, flt_to_int, hypot, int_to_chr, int_to_flt, is_inf, is_nan,
    len, ln, log10, log2, max, min, mod_int, parse_flt, pop_count, pow, pow_float, rotl, rotr,
    saturating_add, saturating_mul, saturating_sub, shl, shr, sin, sqrt, tan, wrapping_add,
    wrapping_mul, wrapping_sub, FolLength,
};
pub use crate::containers::{
    clear_map, clear_vec, contains_map, get_map, index_array, index_seq, index_set, index_vec,
    insert_map, insert_vec, keys_map, lookup_map, pop_vec, push_vec, remove_map, remove_vec,
    render_array, render_map, render_seq, render_set, render_vec, reserve_vec, slice_seq,
    slice_vec, sort_vec, store_array, store_vec, swap_vec, truncate_vec, values_map, FolArray,
};
pub use crate::error::{assert_message, assert_that, require};
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

/// Char-indexed access. `.len`, `str_sub`, `str_find` and `str_byte` are the
/// BYTE world; these are the char world. `str_char_index` is the only sanctioned
/// bridge between them. Mixing the two silently mangles multi-byte text — the
/// bug that made `to_upper("héllo")` return `"HLLO"`.
pub fn str_char_len(text: FolStr) -> crate::value::FolInt {
    text.as_str().chars().count() as crate::value::FolInt
}

pub fn str_byte_len(text: FolStr) -> crate::value::FolInt {
    text.as_str().len() as crate::value::FolInt
}

/// The character at a char index. Out of range faults, matching an indexed read
/// rather than `str_byte`'s -1: there is no `chr` sentinel to return.
pub fn str_char(text: FolStr, index: crate::value::FolInt) -> crate::value::FolChar {
    let source = text.as_str();
    usize::try_from(index)
        .ok()
        .and_then(|index| source.chars().nth(index))
        .unwrap_or_else(|| {
            panic!(
                "fol runtime fault: char index out of bounds: the char len is {} but the index is {index}",
                source.chars().count()
            );
        })
}

/// The BYTE offset where char `index` begins, so a char position can be handed
/// to `str_sub`. `index == char_len` yields the byte length, which makes it
/// usable as an exclusive end bound.
pub fn str_char_index(text: FolStr, index: crate::value::FolInt) -> crate::value::FolInt {
    let source = text.as_str();
    let Ok(index) = usize::try_from(index) else {
        return -1;
    };
    source
        .char_indices()
        .map(|(offset, _)| offset)
        .chain(std::iter::once(source.len()))
        .nth(index)
        .map_or(-1, |offset| offset as crate::value::FolInt)
}

pub fn str_chars(text: FolStr) -> FolVec<crate::value::FolChar> {
    FolVec::from_items(text.as_str().chars().collect())
}

pub fn str_from_chars(chars: FolVec<crate::value::FolChar>) -> FolStr {
    FolStr::new(chars.as_slice().iter().collect::<String>())
}

/// Whether the bytes are well-formed UTF-8. A `FolStr` built inside FOL always
/// is; one read from a socket or a file may not be.
pub fn str_valid_utf8(text: FolStr) -> crate::value::FolBool {
    std::str::from_utf8(text.as_str().as_bytes()).is_ok()
}

/// How many threads can actually run at once. Spawning more workers than this
/// adds scheduling cost without adding throughput, so it is what a worker pool
/// should size itself from. Falls back to 1 when the OS will not say.
pub fn cpu_count() -> crate::value::FolInt {
    std::thread::available_parallelism().map_or(1, |count| count.get() as crate::value::FolInt)
}

/// Hand the rest of this time slice back. A spin loop without it starves the
/// thread it is waiting on, which on a single core means it never finishes.
pub fn thread_yield() -> crate::value::FolInt {
    std::thread::yield_now();
    0
}

/// A small dense integer per thread. Rust's `ThreadId` is opaque and not an
/// integer, so ids are handed out in first-call order — stable within a run,
/// meaningless across runs, which is all a log line needs.
pub fn thread_id() -> crate::value::FolInt {
    static NEXT: std::sync::atomic::AtomicI64 = std::sync::atomic::AtomicI64::new(1);
    thread_local! {
        static ID: std::cell::Cell<i64> = const { std::cell::Cell::new(0) };
    }
    ID.with(|slot| {
        if slot.get() == 0 {
            slot.set(NEXT.fetch_add(1, Ordering::Relaxed));
        }
        slot.get()
    })
}

/// Shared counters as integer handles, the same shape as sockets. A `mux[T]`
/// would also work and is the right tool for compound state, but a lock per
/// counter is heavy when the only operation is an increment — that is exactly
/// what atomics are for.
///
/// The `Arc` is cloned OUT of the registry before the operation, so the map lock
/// is never held across one. Consistent with the socket rule, and it keeps
/// contention on the counter rather than on the registry.
fn atomics() -> &'static Mutex<
    std::collections::HashMap<crate::value::FolInt, Arc<std::sync::atomic::AtomicI64>>,
> {
    static ATOMICS: OnceLock<
        Mutex<std::collections::HashMap<crate::value::FolInt, Arc<std::sync::atomic::AtomicI64>>>,
    > = OnceLock::new();
    ATOMICS.get_or_init(|| Mutex::new(std::collections::HashMap::new()))
}

fn atomic_slot(handle: crate::value::FolInt) -> Option<Arc<std::sync::atomic::AtomicI64>> {
    atomics()
        .lock()
        .unwrap_or_else(|error| error.into_inner())
        .get(&handle)
        .cloned()
}

pub fn atomic_new(initial: crate::value::FolInt) -> crate::value::FolInt {
    static NEXT: std::sync::atomic::AtomicI64 = std::sync::atomic::AtomicI64::new(1);
    let handle = NEXT.fetch_add(1, Ordering::Relaxed);
    atomics()
        .lock()
        .unwrap_or_else(|error| error.into_inner())
        .insert(handle, Arc::new(std::sync::atomic::AtomicI64::new(initial)));
    handle
}

pub fn atomic_load(handle: crate::value::FolInt) -> crate::value::FolInt {
    atomic_slot(handle).map_or(0, |slot| slot.load(Ordering::SeqCst))
}

pub fn atomic_store(
    handle: crate::value::FolInt,
    value: crate::value::FolInt,
) -> crate::value::FolInt {
    match atomic_slot(handle) {
        Some(slot) => {
            slot.store(value, Ordering::SeqCst);
            0
        }
        None => -1,
    }
}

/// Adds and returns the value BEFORE the addition, so concurrent callers each
/// get a distinct number — that is what makes it usable as a ticket dispenser.
pub fn atomic_add(
    handle: crate::value::FolInt,
    delta: crate::value::FolInt,
) -> crate::value::FolInt {
    atomic_slot(handle).map_or(0, |slot| slot.fetch_add(delta, Ordering::SeqCst))
}

/// Compare-and-swap. Returns the value that was actually there: equal to
/// `expected` means the swap happened, anything else means it did not and tells
/// the caller what to retry against.
pub fn atomic_cas(
    handle: crate::value::FolInt,
    expected: crate::value::FolInt,
    desired: crate::value::FolInt,
) -> crate::value::FolInt {
    let Some(slot) = atomic_slot(handle) else {
        return -1;
    };
    match slot.compare_exchange(expected, desired, Ordering::SeqCst, Ordering::SeqCst) {
        Ok(previous) => previous,
        Err(actual) => actual,
    }
}

/// Compares two byte strings in time that depends only on their length, never
/// on where they first differ. The ordinary `==` returns as soon as it finds a
/// mismatching byte, so an attacker who can time the comparison learns how many
/// leading bytes they guessed right and can recover a secret byte by byte. No
/// amount of care in FOL fixes that — a FOL loop compiles to the same early
/// exit — which is why this has to be an intrinsic.
///
/// Length is treated as public, as it is in every constant-time comparison:
/// unequal lengths return immediately. Use this for tokens, MACs, and password
/// hashes; use `==` for everything else, since this is slower by design.
pub fn bytes_equal_ct(left: FolStr, right: FolStr) -> crate::value::FolBool {
    let left = left.as_str().as_bytes();
    let right = right.as_str().as_bytes();
    if left.len() != right.len() {
        return false;
    }
    let mut difference = 0u8;
    for (l, r) in left.iter().zip(right.iter()) {
        difference |= l ^ r;
    }
    // Keeps the optimizer from proving the accumulator can be tested early and
    // reintroducing the branch this whole routine exists to avoid.
    std::hint::black_box(difference) == 0
}

const SIPHASH_KEY_LOW: u64 = 0;
const SIPHASH_KEY_HIGH: u64 = 0;

fn siphash_round(v: &mut [u64; 4]) {
    v[0] = v[0].wrapping_add(v[1]);
    v[1] = v[1].rotate_left(13);
    v[1] ^= v[0];
    v[0] = v[0].rotate_left(32);
    v[2] = v[2].wrapping_add(v[3]);
    v[3] = v[3].rotate_left(16);
    v[3] ^= v[2];
    v[0] = v[0].wrapping_add(v[3]);
    v[3] = v[3].rotate_left(21);
    v[3] ^= v[0];
    v[2] = v[2].wrapping_add(v[1]);
    v[1] = v[1].rotate_left(17);
    v[1] ^= v[2];
    v[2] = v[2].rotate_left(32);
}

/// SipHash-2-4, written out rather than delegated to Rust's `DefaultHasher`,
/// which is explicitly documented as unstable across Rust releases. A hash a
/// program can persist or shard on must not change when the compiler is
/// upgraded, so the algorithm is pinned here.
fn siphash24(key_low: u64, key_high: u64, data: &[u8]) -> u64 {
    let mut v: [u64; 4] = [
        key_low ^ 0x736f_6d65_7073_6575,
        key_high ^ 0x646f_7261_6e64_6f6d,
        key_low ^ 0x6c79_6765_6e65_7261,
        key_high ^ 0x7465_6462_7974_6573,
    ];
    let mut chunks = data.chunks_exact(8);
    for chunk in chunks.by_ref() {
        let word = u64::from_le_bytes(chunk.try_into().expect("chunks_exact(8) yields 8 bytes"));
        v[3] ^= word;
        siphash_round(&mut v);
        siphash_round(&mut v);
        v[0] ^= word;
    }
    // The final word packs the tail below the length's low byte, which is what
    // makes "ab" and "ab\0" hash differently.
    let tail = chunks.remainder();
    let mut last = (data.len() as u64 & 0xff) << 56;
    for (index, byte) in tail.iter().enumerate() {
        last |= u64::from(*byte) << (8 * index);
    }
    v[3] ^= last;
    siphash_round(&mut v);
    siphash_round(&mut v);
    v[0] ^= last;
    v[2] ^= 0xff;
    for _ in 0..4 {
        siphash_round(&mut v);
    }
    v[0] ^ v[1] ^ v[2] ^ v[3]
}

/// A stable 64-bit hash of a string's bytes. The same input gives the same
/// number in every run and every build, so it is safe to persist or to shard
/// on — unlike the hash behind `map[K,V]`, which is randomly keyed per process.
///
/// That stability is also the limitation: the key is fixed, so this is NOT
/// resistant to an attacker who chooses inputs to collide, and it is not a
/// cryptographic digest. Use `std::hash` for SHA-256 when the hash must resist
/// an adversary.
///
/// The result covers the full 64-bit range and is therefore often negative.
pub fn hash_bytes(text: FolStr) -> crate::value::FolInt {
    siphash24(SIPHASH_KEY_LOW, SIPHASH_KEY_HIGH, text.as_str().as_bytes()) as crate::value::FolInt
}

/// The call stack at this point, captured regardless of `RUST_BACKTRACE`.
///
/// Frames carry the emitted Rust symbol names, so FOL routines appear under
/// their generated spelling rather than their source spelling, and inlining may
/// drop frames entirely in an optimized build. It is a debugging aid, not
/// something to parse.
pub fn backtrace() -> FolStr {
    FolStr::new(std::backtrace::Backtrace::force_capture().to_string())
}

// The reason the last hosted call failed.
//
// Every fallible intrinsic here reports failure as a sentinel — `-1`, or an
// empty string — which says that something went wrong and nothing about what.
// A program could not tell a missing file from an unreadable one, so it could
// not print the message a user needs. This is errno's shape: the call reports
// success or failure, and the reason is fetched afterwards.
//
// Thread-local, because two threads failing at once must not overwrite each
// other's reason. Cleared on success, so a stale reason is never read as a
// fresh one.
thread_local! {
    static LAST_OS_ERROR: std::cell::RefCell<Option<(crate::value::FolInt, String)>> =
        const { std::cell::RefCell::new(None) };
}

/// Stable codes, so a program can branch on the reason instead of matching the
/// message text. The numbers are part of the surface and must not be renumbered;
/// `std::os` exports them by name.
fn os_error_code(kind: std::io::ErrorKind) -> crate::value::FolInt {
    use std::io::ErrorKind;
    match kind {
        ErrorKind::NotFound => 1,
        ErrorKind::PermissionDenied => 2,
        ErrorKind::AlreadyExists => 3,
        ErrorKind::ConnectionRefused => 4,
        ErrorKind::ConnectionReset => 5,
        ErrorKind::ConnectionAborted => 6,
        ErrorKind::NotConnected => 7,
        ErrorKind::AddrInUse => 8,
        ErrorKind::AddrNotAvailable => 9,
        ErrorKind::BrokenPipe => 10,
        ErrorKind::WouldBlock => 11,
        ErrorKind::TimedOut => 12,
        ErrorKind::InvalidInput => 13,
        ErrorKind::InvalidData => 14,
        ErrorKind::UnexpectedEof => 15,
        ErrorKind::Interrupted => 16,
        ErrorKind::WriteZero => 17,
        // Everything else, including the kinds still unstable in Rust, so that
        // a toolchain upgrade cannot silently change an existing code.
        _ => 99,
    }
}

pub(crate) fn note_os_error(error: &std::io::Error) {
    let recorded = (os_error_code(error.kind()), error.to_string());
    LAST_OS_ERROR.with(|slot| *slot.borrow_mut() = Some(recorded));
}

pub(crate) fn clear_os_error() {
    LAST_OS_ERROR.with(|slot| *slot.borrow_mut() = None);
}

/// The `()`-returning form: 0 on success, -1 on failure.
fn report_unit(result: std::io::Result<()>) -> crate::value::FolInt {
    match result {
        Ok(()) => {
            clear_os_error();
            0
        }
        Err(error) => {
            note_os_error(&error);
            -1
        }
    }
}

/// The last failure's message, or `""` when the last hosted call succeeded.
pub fn os_error() -> FolStr {
    LAST_OS_ERROR.with(|slot| match slot.borrow().as_ref() {
        Some((_, message)) => FolStr::new(message.as_str()),
        None => FolStr::new(""),
    })
}

/// The last failure's stable code, or 0 when the last hosted call succeeded.
pub fn os_error_kind() -> crate::value::FolInt {
    LAST_OS_ERROR.with(|slot| slot.borrow().as_ref().map_or(0, |(code, _)| *code))
}

// Seconds to add to UTC to get local time, e.g. 7200 for CEST.
//
// `time_parts` works in UTC, so without this every timestamp a program prints
// is wrong for its reader. The offset is read by asking the C library rather
// than parsing TZif ourselves: `localtime_r` applies the whole zone database
// including the DST rule that applies at that instant, which is the part a
// hand-rolled reader gets wrong.
//
// Takes the instant because the offset is not a constant — the same zone is
// +3600 in January and +7200 in July.
extern "C" {
    fn localtime_r(time: *const i64, result: *mut CTm) -> *mut CTm;
    // `localtime_r` deliberately does NOT load the zone, unlike `localtime` —
    // glibc leaves that to the caller. Without this the zone is never read and
    // every offset comes back 0, which looks exactly like a correct answer on a
    // UTC machine. Called every time because TZ can change under the process.
    fn tzset();
}

#[repr(C)]
#[derive(Default)]
struct CTm {
    sec: i32,
    min: i32,
    hour: i32,
    mday: i32,
    mon: i32,
    year: i32,
    wday: i32,
    yday: i32,
    isdst: i32,
    gmtoff: i64,
    zone: *const i8,
}

pub fn tz_offset_sec(epoch_secs: crate::value::FolInt) -> crate::value::FolInt {
    let when: i64 = epoch_secs;
    // SAFETY: loads the zone from TZ or /etc/localtime; safe to call repeatedly.
    unsafe { tzset() };
    let mut parts = CTm {
        zone: std::ptr::null(),
        ..Default::default()
    };
    // SAFETY: `localtime_r` writes into the caller's struct and takes no
    // ownership; the `_r` form is the reentrant one, so no shared static is
    // touched and concurrent calls are safe.
    let filled = unsafe { localtime_r(&when as *const i64, &mut parts as *mut CTm) };
    if filled.is_null() {
        return 0;
    }
    parts.gmtoff
}

/// The absolute path with symlinks and `..` resolved on disk.
///
/// `std::path::normalize` resolves `..` textually, which is a different answer
/// whenever a symlink is involved and therefore cannot decide whether a path
/// stays inside an allowed directory. This asks the filesystem, so the path
/// must exist. Empty string on failure, with the reason in `os_error`.
pub fn realpath(path: FolStr) -> FolStr {
    match std::fs::canonicalize(path.as_str()) {
        Ok(resolved) => {
            clear_os_error();
            FolStr::new(resolved.to_string_lossy().into_owned())
        }
        Err(error) => {
            note_os_error(&error);
            FolStr::new("")
        }
    }
}

/// Creates a new empty file with a unique name in the temp directory and
/// returns its path.
///
/// The point is the create: writing a config safely means writing a temp file
/// and renaming it over the target, and `rename_file` already exists. Choosing
/// a name in FOL and then opening it would race — another process could take
/// the name in between — so the create has to be exclusive, which is what this
/// does.
pub fn temp_file(prefix: FolStr) -> FolStr {
    let mut attempt = 0u32;
    loop {
        let candidate = std::env::temp_dir().join(format!(
            "{}{}-{}-{}",
            prefix.as_str(),
            std::process::id(),
            attempt,
            mono_ns()
        ));
        match std::fs::OpenOptions::new()
            .write(true)
            // The exclusive create is what makes the name ours: it fails rather
            // than opening a file somebody else just made.
            .create_new(true)
            .open(&candidate)
        {
            Ok(_) => {
                clear_os_error();
                return FolStr::new(candidate.to_string_lossy().into_owned());
            }
            Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists && attempt < 64 => {
                attempt += 1;
            }
            Err(error) => {
                note_os_error(&error);
                return FolStr::new("");
            }
        }
    }
}

// Advisory whole-file lock on an open handle, via `flock`.
//
// This is how a program stays single-instance: take an exclusive lock on a
// known path and exit if another process already holds it. `exclusive` false
// takes a shared (read) lock. `wait` false returns -1 immediately rather than
// blocking, so a caller can report "already running" instead of hanging.
//
// Advisory means it only binds processes that also ask; it does not stop an
// unrelated writer.
extern "C" {
    fn flock(fd: i32, operation: i32) -> i32;
}

/// Read under the lock rather than through `open_file_slot`, which hands back a
/// CLONE — the clone's descriptor is closed the moment it drops, so locking it
/// applied to an already-closed fd and failed. `flock` only needs the number,
/// and the registry keeps the file open behind it.
fn flock_fd(handle: crate::value::FolInt) -> Option<i32> {
    use std::os::fd::AsRawFd;
    open_files()
        .lock()
        .unwrap_or_else(|error| error.into_inner())
        .get(&handle)
        .map(|file| file.as_raw_fd())
}

pub fn file_lock(
    handle: crate::value::FolInt,
    exclusive: crate::value::FolBool,
    wait: crate::value::FolBool,
) -> crate::value::FolInt {
    const LOCK_SH: i32 = 1;
    const LOCK_EX: i32 = 2;
    const LOCK_NB: i32 = 4;
    let Some(fd) = flock_fd(handle) else {
        return -1;
    };
    let mut operation = if exclusive { LOCK_EX } else { LOCK_SH };
    if !wait {
        operation |= LOCK_NB;
    }
    // SAFETY: `fd` comes from a live `File` in the registry, so it is open for
    // the duration of the call.
    if unsafe { flock(fd, operation) } == 0 {
        clear_os_error();
        0
    } else {
        note_os_error(&std::io::Error::last_os_error());
        -1
    }
}

pub fn file_unlock(handle: crate::value::FolInt) -> crate::value::FolInt {
    const LOCK_UN: i32 = 8;
    let Some(fd) = flock_fd(handle) else {
        return -1;
    };
    // SAFETY: as `file_lock`.
    if unsafe { flock(fd, LOCK_UN) } == 0 {
        clear_os_error();
        0
    } else {
        note_os_error(&std::io::Error::last_os_error());
        -1
    }
}

/// Creates a symbolic link at `link` pointing to `target`.
///
/// `read_link` could already follow one; nothing could make one, which made the
/// surface asymmetric and left any program that manages a `current -> release`
/// symlink unwritable.
pub fn make_symlink(target: FolStr, link: FolStr) -> crate::value::FolInt {
    report_unit(std::os::unix::fs::symlink(target.as_str(), link.as_str()))
}

/// Removes an environment variable, which setting it to `""` does not do: an
/// empty value is still a present variable, and a child process can tell the
/// difference.
pub fn unset_env_var(name: FolStr) -> crate::value::FolInt {
    std::env::remove_var(name.as_str());
    clear_os_error();
    0
}

/// Long-running child processes, addressed by handle like sockets and files.
///
/// `run_capture`, `run_status` and `run_input` all block until the child exits,
/// which is right for a filter and useless for anything supervised: you cannot
/// start a server, watch it, and stop it. These separate starting from waiting.
///
/// Standard streams are inherited, so the child shares this program's terminal.
/// Capturing output *and* supervising at once would need a reader thread per
/// stream; `run_capture` remains the answer when the output is what you want.
/// A finished child keeps its status instead of vanishing. Polling with
/// `child_try_wait` until it reports done and then calling `child_wait` is the
/// obvious way to supervise, and removing the entry on completion made that
/// second call fail with "unknown handle". So the slot flips to `Done` and every
/// later read returns the same status.
enum ChildSlot {
    Live(std::process::Child),
    Done(crate::value::FolInt),
}

fn children() -> &'static Mutex<std::collections::HashMap<crate::value::FolInt, ChildSlot>> {
    static CHILDREN: OnceLock<Mutex<std::collections::HashMap<crate::value::FolInt, ChildSlot>>> =
        OnceLock::new();
    CHILDREN.get_or_init(|| Mutex::new(std::collections::HashMap::new()))
}

pub fn child_spawn(program: FolStr, args: FolVec<FolStr>) -> crate::value::FolInt {
    static NEXT: std::sync::atomic::AtomicI64 = std::sync::atomic::AtomicI64::new(1);
    let mut command = std::process::Command::new(program.as_str());
    for arg in args.as_slice() {
        command.arg(arg.as_str());
    }
    match command.spawn() {
        Ok(child) => {
            clear_os_error();
            let handle = NEXT.fetch_add(1, Ordering::Relaxed);
            children()
                .lock()
                .unwrap_or_else(|error| error.into_inner())
                .insert(handle, ChildSlot::Live(child));
            handle
        }
        Err(error) => {
            note_os_error(&error);
            -1
        }
    }
}

/// The child's process id, so a caller can log it or reach it by other means.
pub fn child_pid(handle: crate::value::FolInt) -> crate::value::FolInt {
    children()
        .lock()
        .unwrap_or_else(|error| error.into_inner())
        .get(&handle)
        .map_or(-1, |slot| match slot {
            ChildSlot::Live(child) => child.id() as crate::value::FolInt,
            // Already exited, so there is no live process to name.
            ChildSlot::Done(_) => -1,
        })
}

/// Exit status if the child has finished, or -2 while it is still running.
///
/// -2 rather than -1 so "not done yet" is distinguishable from "no such
/// handle", which is the mistake a poll loop would otherwise make.
pub fn child_try_wait(handle: crate::value::FolInt) -> crate::value::FolInt {
    let mut registry = children().lock().unwrap_or_else(|error| error.into_inner());
    let Some(slot) = registry.get_mut(&handle) else {
        return -1;
    };
    let child = match slot {
        ChildSlot::Live(child) => child,
        ChildSlot::Done(code) => return *code,
    };
    match child.try_wait() {
        Ok(Some(status)) => {
            clear_os_error();
            let code = status.code().unwrap_or(-1) as crate::value::FolInt;
            *slot = ChildSlot::Done(code);
            code
        }
        Ok(None) => -2,
        Err(error) => {
            note_os_error(&error);
            -1
        }
    }
}

/// Blocks until the child exits and yields its status. The handle is released,
/// so the child is never left as a zombie.
pub fn child_wait(handle: crate::value::FolInt) -> crate::value::FolInt {
    // Taken out of the registry before waiting, so a blocking wait does not
    // hold the lock against every other child operation — the rule the socket
    // deadlock taught.
    let taken = children()
        .lock()
        .unwrap_or_else(|error| error.into_inner())
        .remove(&handle);
    let mut child = match taken {
        Some(ChildSlot::Live(child)) => child,
        // Already finished and reaped by a poll; the status was kept.
        Some(ChildSlot::Done(code)) => {
            clear_os_error();
            return code;
        }
        None => return -1,
    };
    match child.wait() {
        Ok(status) => {
            clear_os_error();
            status.code().unwrap_or(-1) as crate::value::FolInt
        }
        Err(error) => {
            note_os_error(&error);
            -1
        }
    }
}

/// Sends a signal to the child. `signum` 0 uses `SIGKILL`, which cannot be
/// caught; any other number is sent as given, so `15` asks politely and lets
/// the child run its own cleanup.
pub fn child_kill(
    handle: crate::value::FolInt,
    signum: crate::value::FolInt,
) -> crate::value::FolInt {
    let pid = child_pid(handle);
    if pid < 0 {
        return -1;
    }
    extern "C" {
        fn kill(pid: i32, sig: i32) -> i32;
    }
    let sig = if signum == 0 { 9 } else { signum as i32 };
    // SAFETY: the pid comes from a live `Child` still held in the registry, so
    // it has not been reaped and cannot have been reused.
    if unsafe { kill(pid as i32, sig) } == 0 {
        clear_os_error();
        0
    } else {
        note_os_error(&std::io::Error::last_os_error());
        -1
    }
}

/// Which of the given socket handles have data ready, waiting up to
/// `timeout_ms`. An empty result means the timeout expired.
///
/// Without this, serving several connections means a thread per connection —
/// which FOL can do, and which stops scaling once the connections outnumber
/// what the scheduler should carry. This is the other shape: one thread, many
/// sockets.
///
/// A negative timeout waits indefinitely.
pub fn poll_read(
    handles: FolVec<crate::value::FolInt>,
    timeout_ms: crate::value::FolInt,
) -> FolVec<crate::value::FolInt> {
    #[repr(C)]
    #[derive(Clone, Copy)]
    struct PollFd {
        fd: i32,
        events: i16,
        revents: i16,
    }
    extern "C" {
        fn poll(fds: *mut PollFd, nfds: u64, timeout: i32) -> i32;
    }
    const POLLIN: i16 = 0x001;

    let requested = handles.as_slice();
    let mut slots: Vec<PollFd> = Vec::with_capacity(requested.len());
    let mut owners: Vec<crate::value::FolInt> = Vec::with_capacity(requested.len());
    for handle in requested {
        if let Some(fd) = socket_fd(*handle) {
            slots.push(PollFd {
                fd,
                events: POLLIN,
                revents: 0,
            });
            owners.push(*handle);
        }
    }
    if slots.is_empty() {
        return FolVec::from_items(Vec::new());
    }
    // SAFETY: `slots` is a live, correctly sized array of the C struct, and the
    // descriptors come from sockets still held in the registry.
    let ready = unsafe { poll(slots.as_mut_ptr(), slots.len() as u64, timeout_ms as i32) };
    if ready < 0 {
        note_os_error(&std::io::Error::last_os_error());
        return FolVec::from_items(Vec::new());
    }
    clear_os_error();
    FolVec::from_items(
        slots
            .iter()
            .zip(owners.iter())
            .filter(|(slot, _)| slot.revents & POLLIN != 0)
            .map(|(_, handle)| *handle)
            .collect(),
    )
}

/// The path this program was invoked as — argv[0].
///
/// `arg_at(0)` is the FIRST argument, not the program, so a usage message could
/// not name the command and a program could not re-exec itself.
pub fn arg_program() -> FolStr {
    FolStr::new(
        std::env::args_os()
            .next()
            .map(|arg| arg.to_string_lossy().into_owned())
            .unwrap_or_default(),
    )
}

/// Binary file access. `read_file`/`write_file` assume UTF-8, which silently
/// mangles anything that is not text; these carry bytes verbatim.
/// Byte values, clamped into `0..=255`. A FOL `vec[int]` can hold anything, so
/// the out-of-range case has to be decided rather than assumed; it is what
/// makes a byte vector invalid rather than silently truncated.
fn byte_payload(bytes: &FolVec<crate::value::FolInt>) -> Option<Vec<u8>> {
    bytes
        .as_slice()
        .iter()
        .map(|value| u8::try_from(*value).ok())
        .collect()
}

// Signal handling.
//
// `signal` is declared here rather than pulled in through a crate: the runtime
// is linked into every generated FOL binary and carries no dependencies, and
// libc is already linked because `std` is. FOL is Linux-only, so this is not a
// portability shortcut.
//
// The handler does the one thing that is async-signal-safe — set a flag — and
// `signal_pending` reports it later on an ordinary thread. Running FOL code
// inside a handler would not be safe, so the surface is deliberately a poll
// rather than a callback.
extern "C" {
    fn signal(signum: i32, handler: usize) -> usize;
}

const MAX_SIGNAL: usize = 64;

fn signal_flags() -> &'static [AtomicBool; MAX_SIGNAL] {
    static FLAGS: OnceLock<[AtomicBool; MAX_SIGNAL]> = OnceLock::new();
    FLAGS.get_or_init(|| std::array::from_fn(|_| AtomicBool::new(false)))
}

extern "C" fn record_signal(signum: i32) {
    if signum > 0 && (signum as usize) < MAX_SIGNAL {
        signal_flags()[signum as usize].store(true, Ordering::SeqCst);
    }
}

/// Ask for a signal to be recorded instead of killing the process. Returns 0,
/// or -1 if the signal number is out of range or the kernel refused (`SIGKILL`
/// and `SIGSTOP` cannot be caught).
pub fn signal_trap(signum: crate::value::FolInt) -> crate::value::FolInt {
    if signum <= 0 || signum as usize >= MAX_SIGNAL as crate::value::FolInt as usize {
        return -1;
    }
    // Touch the flags before installing, so the handler never races their
    // initialization.
    let _ = signal_flags();
    const SIG_ERR: usize = usize::MAX;
    let previous = unsafe { signal(signum as i32, record_signal as usize) };
    if previous == SIG_ERR {
        -1
    } else {
        0
    }
}

/// The lowest trapped signal that has arrived since the last call, or 0.
/// Consuming it clears the flag, so a delivered signal is reported once.
pub fn signal_pending() -> crate::value::FolInt {
    let flags = signal_flags();
    for (signum, flag) in flags.iter().enumerate().skip(1) {
        if flag.swap(false, Ordering::SeqCst) {
            return signum as crate::value::FolInt;
        }
    }
    0
}

/// Open files, addressed by handle like sockets.
///
/// `read_file`/`read_bytes` load a whole file, so a file larger than memory, or
/// one still being written, cannot be processed at all. These are the streaming
/// form.
///
/// Reads and writes are **bytes**, not text, so the surface is binary-safe;
/// `str_from_bytes` and `str_bytes` bridge to text when the content is UTF-8.
fn open_files() -> &'static Mutex<std::collections::HashMap<crate::value::FolInt, std::fs::File>> {
    static FILES: OnceLock<Mutex<std::collections::HashMap<crate::value::FolInt, std::fs::File>>> =
        OnceLock::new();
    FILES.get_or_init(|| Mutex::new(std::collections::HashMap::new()))
}

/// Clones the descriptor out and drops the registry lock before any I/O, the
/// same rule the socket registry follows: a blocking read on a fifo must not
/// freeze every other file operation in the process. A cloned descriptor shares
/// the kernel file offset, so sequential reads still advance as one stream.
fn open_file_slot(handle: crate::value::FolInt) -> Option<std::fs::File> {
    open_files()
        .lock()
        .unwrap_or_else(|error| error.into_inner())
        .get(&handle)
        .and_then(|file| file.try_clone().ok())
}

/// Modes: 0 read, 1 truncate-or-create, 2 append, 3 read-write without
/// truncating. Returns a handle, or -1.
///
/// An integer rather than `"r"`/`"w"`, which would be the obvious spelling but
/// is unwritable: FOL types a one-character double-quoted literal as `chr`, so
/// `.file_open(path, "w")` does not typecheck. It also matches the selector
/// arguments this surface already uses in `file_seek` and `tcp_shutdown`.
/// `std::fs` wraps these as named routines.
pub fn file_open(path: FolStr, mode: crate::value::FolInt) -> crate::value::FolInt {
    static NEXT: std::sync::atomic::AtomicI64 = std::sync::atomic::AtomicI64::new(1);
    let mut options = std::fs::OpenOptions::new();
    match mode {
        0 => options.read(true),
        1 => options.write(true).create(true).truncate(true),
        2 => options.append(true).create(true),
        3 => options.read(true).write(true).create(true),
        _ => return -1,
    };
    let Ok(file) = options.open(path.as_str()) else {
        return -1;
    };
    let handle = NEXT.fetch_add(1, Ordering::Relaxed);
    open_files()
        .lock()
        .unwrap_or_else(|error| error.into_inner())
        .insert(handle, file);
    handle
}

/// Up to `count` bytes. A short vector means end of file was reached; an empty
/// one means there is nothing left.
pub fn file_read(
    handle: crate::value::FolInt,
    count: crate::value::FolInt,
) -> FolVec<crate::value::FolInt> {
    use std::io::Read;
    if count <= 0 {
        return FolVec::from_items(Vec::new());
    }
    let Some(mut file) = open_file_slot(handle) else {
        return FolVec::from_items(Vec::new());
    };
    let mut buffer = vec![0u8; count as usize];
    match file.read(&mut buffer) {
        Ok(read) => FolVec::from_items(
            buffer[..read]
                .iter()
                .map(|byte| *byte as crate::value::FolInt)
                .collect(),
        ),
        Err(error) => {
            note_os_error(&error);
            FolVec::from_items(Vec::new())
        }
    }
}

/// The bytes written, or -1.
pub fn file_write(
    handle: crate::value::FolInt,
    bytes: FolVec<crate::value::FolInt>,
) -> crate::value::FolInt {
    use std::io::Write;
    let Some(payload) = byte_payload(&bytes) else {
        return -1;
    };
    let Some(mut file) = open_file_slot(handle) else {
        return -1;
    };
    match file.write(&payload) {
        Ok(written) => written as crate::value::FolInt,
        Err(error) => {
            note_os_error(&error);
            -1
        }
    }
}

/// `whence`: 0 from the start, 1 from the current position, 2 from the end.
/// Returns the new absolute position, or -1.
pub fn file_seek(
    handle: crate::value::FolInt,
    offset: crate::value::FolInt,
    whence: crate::value::FolInt,
) -> crate::value::FolInt {
    use std::io::Seek;
    let target = match whence {
        0 => std::io::SeekFrom::Start(offset.max(0) as u64),
        1 => std::io::SeekFrom::Current(offset),
        2 => std::io::SeekFrom::End(offset),
        _ => return -1,
    };
    let Some(mut file) = open_file_slot(handle) else {
        return -1;
    };
    match file.seek(target) {
        Ok(position) => position as crate::value::FolInt,
        Err(error) => {
            note_os_error(&error);
            -1
        }
    }
}

pub fn file_flush(handle: crate::value::FolInt) -> crate::value::FolInt {
    use std::io::Write;
    let Some(mut file) = open_file_slot(handle) else {
        return -1;
    };
    match file.flush() {
        Ok(()) => 0,
        Err(error) => {
            note_os_error(&error);
            -1
        }
    }
}

/// Releases the handle. Buffered data is flushed by the drop; an unclosed
/// handle lives until the process exits.
pub fn file_close(handle: crate::value::FolInt) -> crate::value::FolInt {
    match open_files()
        .lock()
        .unwrap_or_else(|error| error.into_inner())
        .remove(&handle)
    {
        Some(_) => 0,
        None => -1,
    }
}

/// A float as the shortest text that parses back to the identical value.
///
/// `float_to_str` takes a fixed number of decimals, which either loses
/// precision or pads noise, so neither setting round-trips reliably. This is
/// what a serializer wants: `0.1` renders as `0.1`, not `0.100000`, and
/// `parse_flt` returns the same bits. `flt_bits` remains the exact machine
/// form; this is the exact *human-readable* one.
pub fn flt_to_str_exact(value: crate::value::FolFloat) -> FolStr {
    if value.is_nan() {
        return FolStr::new("nan");
    }
    if value.is_infinite() {
        return FolStr::new(if value > 0.0 { "inf" } else { "-inf" });
    }
    FolStr::new(format!("{value}"))
}

/// Run a program with text on its standard input.
///
/// `run_capture` builds on `Command::output()`, which gives the child a null
/// stdin, so a child that reads input sees an immediately-closed stream. That
/// makes every filter-shaped program (`sort`, `sha256sum`, `git hash-object
/// --stdin`) unreachable. Returns the same three-element shape as
/// `run_capture`: status, stdout, stderr.
pub fn run_input(program: FolStr, args: FolVec<FolStr>, input: FolStr) -> FolVec<FolStr> {
    use std::io::Write;
    let mut command = std::process::Command::new(program.as_str());
    for arg in args.as_slice() {
        command.arg(arg.as_str());
    }
    command
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped());
    let failed = || FolVec::from_items(vec![FolStr::new("-1"), FolStr::new(""), FolStr::new("")]);
    let Ok(mut child) = command.spawn() else {
        return failed();
    };
    // The write must finish and the pipe close before waiting: a child that
    // reads to end-of-input never exits while this end stays open, and both
    // sides then block forever.
    if let Some(mut stdin) = child.stdin.take() {
        let _ = stdin.write_all(input.as_str().as_bytes());
    }
    match child.wait_with_output() {
        Ok(output) => FolVec::from_items(vec![
            FolStr::new(output.status.code().unwrap_or(-1).to_string()),
            FolStr::new(String::from_utf8_lossy(&output.stdout).into_owned()),
            FolStr::new(String::from_utf8_lossy(&output.stderr).into_owned()),
        ]),
        Err(_) => failed(),
    }
}

/// Terminal columns occupied by one character: 0, 1, or 2.
///
/// Padding with `.len` counts bytes and padding with `str_char_len` counts
/// codepoints; a terminal aligns by neither. `日本` is 6 bytes, 2 codepoints,
/// and 4 columns, so any table built on the other two measures misaligns.
///
/// This is a range table, which is how `wcwidth` is done everywhere. It is
/// exact for the wide and fullwidth blocks (CJK, Hangul, kana, the common
/// emoji planes) and for the combining ranges listed below, which together
/// cover what a terminal program actually meets. It does **not** consult the
/// full Unicode combining-class data, so a rare combining mark outside those
/// ranges is counted as one column instead of zero; the runtime carries no
/// Unicode tables beyond what Rust's `std` exposes, and `std` exposes no
/// general category.
fn char_columns(value: u32) -> crate::value::FolInt {
    // C0/C1 controls occupy no column and must not be counted as one.
    if value == 0 || (0x1..=0x1f).contains(&value) || (0x7f..=0x9f).contains(&value) {
        return 0;
    }
    const ZERO_WIDTH: &[(u32, u32)] = &[
        (0x0300, 0x036f), // combining diacritical marks
        (0x0483, 0x0489), // combining Cyrillic
        (0x0591, 0x05bd), // Hebrew points
        (0x0610, 0x061a), // Arabic marks
        (0x064b, 0x065f), // Arabic diacritics
        (0x0670, 0x0670),
        (0x06d6, 0x06dc),
        (0x0e31, 0x0e31), // Thai vowel marks
        (0x0e34, 0x0e3a),
        (0x0e47, 0x0e4e),
        (0x1ab0, 0x1aff), // combining diacriticals extended
        (0x1dc0, 0x1dff), // combining diacriticals supplement
        (0x20d0, 0x20f0), // combining marks for symbols
        (0xfe00, 0xfe0f), // variation selectors
        (0xfe20, 0xfe2f), // combining half marks
        (0x200b, 0x200f), // zero-width space and directional marks
    ];
    if ZERO_WIDTH
        .iter()
        .any(|(low, high)| value >= *low && value <= *high)
    {
        return 0;
    }
    const WIDE: &[(u32, u32)] = &[
        (0x1100, 0x115f),   // Hangul Jamo initial consonants
        (0x2e80, 0x303e),   // CJK radicals, Kangxi, CJK symbols
        (0x3041, 0x33ff),   // kana through CJK compatibility
        (0x3400, 0x4dbf),   // CJK extension A
        (0x4e00, 0x9fff),   // CJK unified ideographs
        (0xa000, 0xa4cf),   // Yi
        (0xac00, 0xd7a3),   // Hangul syllables
        (0xf900, 0xfaff),   // CJK compatibility ideographs
        (0xfe10, 0xfe19),   // vertical forms
        (0xfe30, 0xfe6f),   // CJK compatibility forms
        (0xff00, 0xff60),   // fullwidth forms
        (0xffe0, 0xffe6),   // fullwidth signs
        (0x1f300, 0x1f64f), // symbols, pictographs, emoticons
        (0x1f900, 0x1f9ff), // supplemental symbols and pictographs
        (0x20000, 0x2fffd), // CJK extension B and beyond
        (0x30000, 0x3fffd),
    ];
    if WIDE
        .iter()
        .any(|(low, high)| value >= *low && value <= *high)
    {
        return 2;
    }
    1
}

pub fn chr_width(value: crate::value::FolChar) -> crate::value::FolInt {
    char_columns(value as u32)
}

/// The columns a whole string occupies, which is what a padded column width
/// has to be computed from.
pub fn str_width(text: FolStr) -> crate::value::FolInt {
    text.as_str()
        .chars()
        .map(|character| char_columns(character as u32))
        .sum()
}

/// Whether a byte vector decodes as UTF-8. Worth asking before
/// `str_from_bytes`, which cannot distinguish "invalid" from "empty" in its
/// return value alone.
pub fn bytes_valid_utf8(bytes: FolVec<crate::value::FolInt>) -> crate::value::FolBool {
    byte_payload(&bytes).is_some_and(|payload| std::str::from_utf8(&payload).is_ok())
}

/// Unicode normalization forms, for `str_normalize`.
///
/// `é` typed as `e` plus a combining accent and `é` typed precomposed look
/// identical, compare unequal, and report different lengths. Normalizing both
/// sides before comparing is the only way user-entered text behaves the way a
/// reader expects.
///
/// - 0 **NFC** — compose. The web's default and the right choice for storing
///   and comparing text.
/// - 1 **NFD** — decompose. Useful when stripping accents, since it separates
///   the base letter from its marks.
/// - 2 **NFKC** — compose, and fold compatibility variants first: `ﬁ` becomes
///   `fi`, fullwidth `Ａ` becomes `A`. Lossy by design; good for search keys,
///   wrong for text you intend to give back to the user.
/// - 3 **NFKD** — decompose with the same compatibility folding.
///
/// An unknown form returns the input unchanged rather than faulting, matching
/// how the other selector arguments on this surface behave.
pub fn str_normalize(text: FolStr, form: crate::value::FolInt) -> FolStr {
    match crate::normalize::Form::from_selector(form) {
        Some(form) => FolStr::new(crate::normalize::normalize(text.as_str(), form)),
        None => text,
    }
}

/// Whether text is already in a normalization form. Cheaper than normalizing
/// and comparing when the answer is usually yes, which for stored text it is.
pub fn str_is_normalized(text: FolStr, form: crate::value::FolInt) -> crate::value::FolBool {
    if !(0..=3).contains(&form) {
        return false;
    }
    str_normalize(text.clone(), form).as_str() == text.as_str()
}

/// How many leading bytes form complete, valid UTF-8.
///
/// Required by any chunked reader. A fixed-size `file_read` splits multi-byte
/// characters across chunk boundaries, and feeding such a chunk to
/// `str_from_bytes` yields the empty string — so a naive loop silently drops
/// exactly the characters this group set out to preserve. Reading `héllo` two
/// bytes at a time produced `lo`.
///
/// The fix is to decode the valid prefix and carry the remainder into the next
/// chunk. Deciding where that prefix ends means knowing UTF-8 sequence
/// structure, which is the part FOL should not be re-deriving.
/// `std::strn::decoder` wraps the whole pattern.
///
/// Returns 0 when the vector starts with a byte that can never begin a valid
/// sequence, which is genuinely invalid rather than merely incomplete.
pub fn utf8_prefix_len(bytes: FolVec<crate::value::FolInt>) -> crate::value::FolInt {
    let Some(payload) = byte_payload(&bytes) else {
        return 0;
    };
    match std::str::from_utf8(&payload) {
        Ok(_) => payload.len() as crate::value::FolInt,
        // `valid_up_to` is the whole point: it separates "this chunk ends
        // mid-character" from "these bytes are malformed".
        Err(error) => error.valid_up_to() as crate::value::FolInt,
    }
}

/// Bytes back into text.
///
/// This is the only way to reconstruct a string from `read_bytes`,
/// `random_bytes`, or any other byte source: `byte_to_str` handles one byte and
/// so cannot express a multi-byte sequence at all, and `int_to_chr` takes a
/// codepoint rather than a byte. Without this, reading `héllo` as bytes and
/// rebuilding it produced `hllo` — silent data loss.
///
/// Invalid UTF-8, or a value outside `0..=255`, yields the empty string rather
/// than substituting replacement characters, so a caller never acts on
/// half-decoded text by accident. Pair it with `bytes_valid_utf8` when empty
/// input and invalid input have to be told apart.
pub fn str_from_bytes(bytes: FolVec<crate::value::FolInt>) -> FolStr {
    let Some(payload) = byte_payload(&bytes) else {
        return FolStr::new("");
    };
    match std::str::from_utf8(&payload) {
        Ok(text) => FolStr::new(text),
        // Not an OS failure, so it does not touch the last-OS-error slot;
        // `bytes_valid_utf8` is the companion that distinguishes empty from
        // invalid here.
        Err(_) => FolStr::new(""),
    }
}

/// Text as its UTF-8 bytes. The inverse of `str_from_bytes`, and what feeds
/// `write_bytes` or a hash without going through `str_byte` one index at a
/// time.
pub fn str_bytes(text: FolStr) -> FolVec<crate::value::FolInt> {
    FolVec::from_items(
        text.as_str()
            .as_bytes()
            .iter()
            .map(|byte| *byte as crate::value::FolInt)
            .collect(),
    )
}

pub fn read_bytes(path: FolStr) -> FolVec<crate::value::FolInt> {
    FolVec::from_items(
        std::fs::read(path.as_str())
            .unwrap_or_default()
            .into_iter()
            .map(|byte| byte as crate::value::FolInt)
            .collect(),
    )
}

pub fn write_bytes(path: FolStr, bytes: FolVec<crate::value::FolInt>) -> crate::value::FolInt {
    let payload: Vec<u8> = bytes
        .as_slice()
        .iter()
        .map(|value| (*value).clamp(0, 255) as u8)
        .collect();
    report_unit(std::fs::write(path.as_str(), payload))
}

/// One entry name per element. This replaces `dir_list`, which packs everything
/// into a single delimited `str` — a workaround from before `vec[str]` existed.
pub fn dir_entries(path: FolStr) -> FolVec<FolStr> {
    let Ok(entries) = std::fs::read_dir(path.as_str()) else {
        return FolVec::from_items(Vec::new());
    };
    let mut names: Vec<FolStr> = entries
        .filter_map(|entry| entry.ok())
        .map(|entry| FolStr::new(entry.file_name().to_string_lossy().into_owned()))
        .collect();
    // Directory order is filesystem-defined and therefore not reproducible;
    // sorting makes a listing stable across runs and machines.
    names.sort();
    FolVec::from_items(names)
}

pub fn remove_dir_all(path: FolStr) -> crate::value::FolInt {
    report_unit(std::fs::remove_dir_all(path.as_str()))
}

/// Uses `symlink_metadata`, which does NOT follow the link — `is_file`/`is_dir`
/// do follow, so a link to a file reports as a file there and as a link here.
pub fn file_is_link(path: FolStr) -> crate::value::FolBool {
    std::fs::symlink_metadata(path.as_str())
        .map(|meta| meta.file_type().is_symlink())
        .unwrap_or(false)
}

pub fn read_link(path: FolStr) -> FolStr {
    std::fs::read_link(path.as_str()).map_or_else(
        |_| FolStr::new(String::new()),
        |target| FolStr::new(target.display().to_string()),
    )
}

/// Unix mode bits, or -1 when unreadable. FOL is Linux-only, so these are the
/// real permissions rather than a portable approximation.
pub fn permissions(path: FolStr) -> crate::value::FolInt {
    use std::os::unix::fs::PermissionsExt;
    std::fs::metadata(path.as_str())
        .map_or(-1, |meta| meta.permissions().mode() as crate::value::FolInt)
}

pub fn set_permissions(path: FolStr, mode: crate::value::FolInt) -> crate::value::FolInt {
    use std::os::unix::fs::PermissionsExt;
    let Ok(mode) = u32::try_from(mode) else {
        return 1;
    };
    report_unit(std::fs::set_permissions(
        path.as_str(),
        std::fs::Permissions::from_mode(mode),
    ))
}

pub fn temp_dir() -> FolStr {
    FolStr::new(std::env::temp_dir().display().to_string())
}

pub fn home_dir() -> FolStr {
    FolStr::new(std::env::var("HOME").unwrap_or_default())
}

pub fn set_current_dir(path: FolStr) -> crate::value::FolInt {
    report_unit(std::env::set_current_dir(path.as_str()))
}

/// Every variable as `"KEY=VALUE"`. Splitting on the first `=` is the caller's
/// job — a value may itself contain `=`, so only the first one separates.
pub fn env_vars() -> FolVec<FolStr> {
    let mut pairs: Vec<FolStr> = std::env::vars()
        .map(|(key, value)| FolStr::new(format!("{key}={value}")))
        .collect();
    pairs.sort();
    FolVec::from_items(pairs)
}

pub fn set_env_var(name: FolStr, value: FolStr) -> crate::value::FolInt {
    std::env::set_var(name.as_str(), value.as_str());
    0
}

pub fn process_id() -> crate::value::FolInt {
    std::process::id() as crate::value::FolInt
}

/// Run a command with an explicit argument vector and capture everything:
/// `[status, stdout, stderr]`, status rendered as decimal so one call answers
/// every question.
///
/// This does NOT go through a shell, which is the whole point: `shell`/
/// `shell_out` pass a string to `sh -c`, so any argument containing a space,
/// quote or `;` changes the meaning of the command. Prefer this whenever an
/// argument came from outside the program.
pub fn run_capture(program: FolStr, args: FolVec<FolStr>) -> FolVec<FolStr> {
    let mut command = std::process::Command::new(program.as_str());
    for arg in args.as_slice() {
        command.arg(arg.as_str());
    }
    match command.output() {
        Ok(output) => FolVec::from_items(vec![
            FolStr::new(output.status.code().unwrap_or(-1).to_string()),
            FolStr::new(String::from_utf8_lossy(&output.stdout).into_owned()),
            FolStr::new(String::from_utf8_lossy(&output.stderr).into_owned()),
        ]),
        Err(_) => FolVec::from_items(vec![
            FolStr::new("-1".to_string()),
            FolStr::new(String::new()),
            FolStr::new(String::new()),
        ]),
    }
}

/// Status only, with the child's streams inherited so it can talk to the
/// terminal. Also shell-free.
pub fn run_status(program: FolStr, args: FolVec<FolStr>) -> crate::value::FolInt {
    let mut command = std::process::Command::new(program.as_str());
    for arg in args.as_slice() {
        command.arg(arg.as_str());
    }
    command.status().map_or(-1, |status| {
        status.code().unwrap_or(-1) as crate::value::FolInt
    })
}

/// Wall-clock nanoseconds since the Unix epoch. `now_ms` is too coarse to
/// measure anything with; this is the same clock at full resolution.
pub fn now_ns() -> crate::value::FolInt {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |elapsed| elapsed.as_nanos() as crate::value::FolInt)
}

/// A MONOTONIC reading, which is the one to subtract for durations: the wall
/// clock jumps when the system time is set or NTP steps it, and a negative
/// elapsed time is the classic result. The origin is arbitrary, so only
/// differences are meaningful.
pub fn mono_ns() -> crate::value::FolInt {
    static ORIGIN: OnceLock<std::time::Instant> = OnceLock::new();
    let origin = ORIGIN.get_or_init(std::time::Instant::now);
    origin.elapsed().as_nanos() as crate::value::FolInt
}

pub fn sleep_ns(nanos: crate::value::FolInt) -> crate::value::FolInt {
    if nanos > 0 {
        std::thread::sleep(std::time::Duration::from_nanos(nanos as u64));
    }
    0
}

/// Days since the Unix epoch to a civil (year, month, day), and back. This is
/// Howard Hinnant's algorithm: exact for the whole proleptic Gregorian calendar,
/// no lookup tables, no leap-year special cases beyond the arithmetic itself.
/// Rust's std has no calendar conversion at all, so it lives here.
fn civil_from_days(days: i64) -> (i64, i64, i64) {
    let z = days + 719_468;
    let era = if z >= 0 { z } else { z - 146_096 } / 146_097;
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = mp + if mp < 10 { 3 } else { -9 };
    (y + i64::from(m <= 2), m, d)
}

fn days_from_civil(year: i64, month: i64, day: i64) -> i64 {
    let y = year - i64::from(month <= 2);
    let era = if y >= 0 { y } else { y - 399 } / 400;
    let yoe = y - era * 400;
    let doy = (153 * (month + if month > 2 { -3 } else { 9 }) + 2) / 5 + day - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    era * 146_097 + doe - 719_468
}

/// Split epoch milliseconds into civil parts, in UTC:
/// `[year, month, day, hour, minute, second, milli, weekday, yearday]`.
/// Weekday is 0 = Sunday; yearday is 1-based. Milliseconds are included so the
/// value round-trips exactly through `time_from_parts`.
///
/// UTC only. Local time needs the OS timezone database, which is a TZif parse
/// this runtime has no dependency for — see `PLAN.md`.
pub fn time_parts(millis: crate::value::FolInt) -> FolVec<crate::value::FolInt> {
    let days = millis.div_euclid(86_400_000);
    let rest = millis.rem_euclid(86_400_000);
    let (year, month, day) = civil_from_days(days);
    let weekday = (days + 4).rem_euclid(7);
    let yearday = days - days_from_civil(year, 1, 1) + 1;
    FolVec::from_items(vec![
        year,
        month,
        day,
        rest / 3_600_000,
        (rest / 60_000) % 60,
        (rest / 1_000) % 60,
        rest % 1_000,
        weekday,
        yearday,
    ])
}

/// The inverse. Reads `[year, month, day, hour, minute, second, milli]`;
/// trailing entries (weekday, yearday) are ignored so a `time_parts` result can
/// be handed straight back. Missing entries default to the start of the period,
/// so `[2026, 8, 11]` is midnight on that date.
pub fn time_from_parts(parts: FolVec<crate::value::FolInt>) -> crate::value::FolInt {
    let field = |index: usize, fallback: i64| -> i64 {
        parts.as_slice().get(index).copied().unwrap_or(fallback)
    };
    let days = days_from_civil(field(0, 1970), field(1, 1), field(2, 1));
    days * 86_400_000
        + field(3, 0) * 3_600_000
        + field(4, 0) * 60_000
        + field(5, 0) * 1_000
        + field(6, 0)
}

/// OS entropy, read from `/dev/urandom`. FOL is Linux-only, so this is the
/// source rather than a portability shim, and it is what makes a seeded PRNG in
/// FOL possible at all — nothing in the language can invent unpredictability.
///
/// The handle is opened once and reused: opening per call would dominate the
/// cost of drawing a few bytes.
fn entropy_source() -> &'static Mutex<Option<std::fs::File>> {
    static SOURCE: OnceLock<Mutex<Option<std::fs::File>>> = OnceLock::new();
    SOURCE.get_or_init(|| Mutex::new(std::fs::File::open("/dev/urandom").ok()))
}

fn fill_entropy(buffer: &mut [u8]) -> bool {
    use std::io::Read;
    let mut guard = entropy_source()
        .lock()
        .unwrap_or_else(|error| error.into_inner());
    let Some(file) = guard.as_mut() else {
        return false;
    };
    file.read_exact(buffer).is_ok()
}

/// Random bytes as `vec[int]`, NOT `str`: a `str` is UTF-8 by construction and
/// random bytes are not valid UTF-8. Faults when entropy is unavailable, because
/// silently returning zeros would look like working randomness.
pub fn random_bytes(count: crate::value::FolInt) -> FolVec<crate::value::FolInt> {
    if count <= 0 {
        return FolVec::from_items(Vec::new());
    }
    let mut buffer = vec![0u8; count as usize];
    if !fill_entropy(&mut buffer) {
        panic!("fol runtime fault: no entropy source available");
    }
    FolVec::from_items(
        buffer
            .into_iter()
            .map(|byte| byte as crate::value::FolInt)
            .collect(),
    )
}

/// Uniform in `[low, high)`. Rejection sampling, not modulo: `x % range` is
/// biased toward the low end whenever the range does not divide the word evenly,
/// and that bias is exactly what breaks shuffles and samplers.
pub fn random_int(low: crate::value::FolInt, high: crate::value::FolInt) -> crate::value::FolInt {
    if high <= low {
        return low;
    }
    let span = (high as i128 - low as i128) as u128;
    let limit = u128::MAX - (u128::MAX % span);
    let mut buffer = [0u8; 16];
    loop {
        if !fill_entropy(&mut buffer) {
            panic!("fol runtime fault: no entropy source available");
        }
        let draw = u128::from_le_bytes(buffer);
        if draw < limit {
            return (low as i128 + (draw % span) as i128) as crate::value::FolInt;
        }
    }
}

/// Uniform in `[0.0, 1.0)`, built from 53 random bits — the number a `f64`
/// mantissa can hold exactly, so every representable value in the range is
/// reachable and none is favoured.
pub fn random_flt() -> crate::value::FolFloat {
    let mut buffer = [0u8; 8];
    if !fill_entropy(&mut buffer) {
        panic!("fol runtime fault: no entropy source available");
    }
    let draw = u64::from_le_bytes(buffer) >> 11;
    draw as crate::value::FolFloat / ((1u64 << 53) as crate::value::FolFloat)
}

/// Unicode case mapping and categories. These are TABLES, not arithmetic: the
/// ASCII trick of adding 32 is wrong for every alphabet with more than 26
/// letters, and full case mapping is not even per-character (`ß` uppercases to
/// `SS`), which is why the string forms exist alongside the char ones.
pub fn chr_upper(value: crate::value::FolChar) -> crate::value::FolChar {
    let mut mapped = value.to_uppercase();
    match (mapped.next(), mapped.next()) {
        // A char-to-char API cannot represent a one-to-many mapping; leave those
        // unchanged rather than truncate, and let `str_upper` handle them.
        (Some(single), None) => single,
        _ => value,
    }
}

pub fn chr_lower(value: crate::value::FolChar) -> crate::value::FolChar {
    let mut mapped = value.to_lowercase();
    match (mapped.next(), mapped.next()) {
        (Some(single), None) => single,
        _ => value,
    }
}

pub fn str_upper(text: FolStr) -> FolStr {
    FolStr::new(text.as_str().to_uppercase())
}

pub fn str_lower(text: FolStr) -> FolStr {
    FolStr::new(text.as_str().to_lowercase())
}

pub fn chr_is_alpha(value: crate::value::FolChar) -> crate::value::FolBool {
    value.is_alphabetic()
}

pub fn chr_is_digit(value: crate::value::FolChar) -> crate::value::FolBool {
    value.is_numeric()
}

pub fn chr_is_space(value: crate::value::FolChar) -> crate::value::FolBool {
    value.is_whitespace()
}

/// Trims the Unicode whitespace set, not just ASCII 32/9/10/13.
pub fn str_trim(text: FolStr) -> FolStr {
    FolStr::new(text.as_str().trim())
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
    match std::fs::read_to_string(path.as_str()) {
        Ok(text) => {
            clear_os_error();
            FolStr::new(text)
        }
        Err(error) => {
            note_os_error(&error);
            FolStr::new("")
        }
    }
}

/// Writes text to a path: 0 on success, -1 when the write fails.
pub fn write_file(path: FolStr, contents: FolStr) -> crate::value::FolInt {
    match std::fs::write(path.as_str(), contents.as_str()) {
        Ok(()) => 0,
        Err(error) => {
            note_os_error(&error);
            -1
        }
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
/// TCP sockets are handed to FOL as integer handles rather than opaque values:
/// FOL has no foreign-handle type, and an `int` keyed into a runtime registry
/// needs no language surface at all. A handle is valid until `tcp_close`;
/// every routine returns -1 (or the empty string) rather than faulting, because
/// a peer disappearing is an ordinary event a server must handle, not a bug.
enum SocketSlot {
    Listener(std::net::TcpListener),
    Stream(std::net::TcpStream),
    Datagram(std::net::UdpSocket),
}

fn sockets() -> &'static Mutex<std::collections::HashMap<crate::value::FolInt, SocketSlot>> {
    static SOCKETS: OnceLock<Mutex<std::collections::HashMap<crate::value::FolInt, SocketSlot>>> =
        OnceLock::new();
    SOCKETS.get_or_init(|| Mutex::new(std::collections::HashMap::new()))
}

fn register_socket(slot: SocketSlot) -> crate::value::FolInt {
    static NEXT: std::sync::atomic::AtomicI64 = std::sync::atomic::AtomicI64::new(1);
    let id = NEXT.fetch_add(1, Ordering::Relaxed);
    sockets()
        .lock()
        .unwrap_or_else(|error| error.into_inner())
        .insert(id, slot);
    id
}

pub fn tcp_listen(address: FolStr) -> crate::value::FolInt {
    match std::net::TcpListener::bind(address.as_str()) {
        Ok(listener) => register_socket(SocketSlot::Listener(listener)),
        Err(error) => {
            note_os_error(&error);
            -1
        }
    }
}

/// Every blocking call clones its socket out of the registry and releases the
/// lock BEFORE blocking. Holding it across `accept`/`read` deadlocks instantly:
/// the blocked thread owns the map that the peer needs in order to register its
/// own handle.
/// The raw descriptor for any socket kind, for `poll_read`.
///
/// Read under the lock and returned as a plain int rather than cloning the
/// socket: `poll` only inspects readiness, so there is nothing to own, and the
/// descriptor stays valid because the registry still holds the socket.
fn socket_fd(handle: crate::value::FolInt) -> Option<i32> {
    use std::os::fd::AsRawFd;
    let guard = sockets().lock().unwrap_or_else(|error| error.into_inner());
    guard.get(&handle).map(|slot| match slot {
        SocketSlot::Listener(listener) => listener.as_raw_fd(),
        SocketSlot::Stream(stream) => stream.as_raw_fd(),
        SocketSlot::Datagram(socket) => socket.as_raw_fd(),
    })
}

fn clone_listener(handle: crate::value::FolInt) -> Option<std::net::TcpListener> {
    let guard = sockets().lock().unwrap_or_else(|error| error.into_inner());
    match guard.get(&handle) {
        Some(SocketSlot::Listener(listener)) => listener.try_clone().ok(),
        _ => None,
    }
}

fn clone_stream(handle: crate::value::FolInt) -> Option<std::net::TcpStream> {
    let guard = sockets().lock().unwrap_or_else(|error| error.into_inner());
    match guard.get(&handle) {
        Some(SocketSlot::Stream(stream)) => stream.try_clone().ok(),
        _ => None,
    }
}

/// Blocks until a peer connects. The accepted stream gets its own handle; the
/// listener stays open for the next call.
pub fn tcp_accept(handle: crate::value::FolInt) -> crate::value::FolInt {
    let Some(listener) = clone_listener(handle) else {
        return -1;
    };
    match listener.accept() {
        Ok((stream, _)) => register_socket(SocketSlot::Stream(stream)),
        Err(error) => {
            note_os_error(&error);
            -1
        }
    }
}

pub fn tcp_connect(address: FolStr) -> crate::value::FolInt {
    match std::net::TcpStream::connect(address.as_str()) {
        Ok(stream) => register_socket(SocketSlot::Stream(stream)),
        Err(error) => {
            note_os_error(&error);
            -1
        }
    }
}

/// Up to 64 KiB of whatever has arrived. An empty string means the peer closed
/// or the handle is not a stream, which is the same terminating condition a
/// read loop already tests for.
pub fn tcp_read(handle: crate::value::FolInt) -> FolStr {
    use std::io::Read;
    let Some(mut stream) = clone_stream(handle) else {
        return FolStr::new(String::new());
    };
    let mut buffer = vec![0u8; 65536];
    match stream.read(&mut buffer) {
        Ok(0) | Err(_) => FolStr::new(String::new()),
        Ok(read) => FolStr::new(String::from_utf8_lossy(&buffer[..read]).into_owned()),
    }
}

pub fn tcp_write(handle: crate::value::FolInt, payload: FolStr) -> crate::value::FolInt {
    use std::io::Write;
    let Some(mut stream) = clone_stream(handle) else {
        return -1;
    };
    stream
        .write_all(payload.as_str().as_bytes())
        .and_then(|()| stream.flush())
        .map_or(-1, |()| 0)
}

pub fn tcp_close(handle: crate::value::FolInt) -> crate::value::FolInt {
    let removed = sockets()
        .lock()
        .unwrap_or_else(|error| error.into_inner())
        .remove(&handle);
    if removed.is_some() {
        0
    } else {
        -1
    }
}

fn clone_datagram(handle: crate::value::FolInt) -> Option<std::net::UdpSocket> {
    let guard = sockets().lock().unwrap_or_else(|error| error.into_inner());
    match guard.get(&handle) {
        Some(SocketSlot::Datagram(socket)) => socket.try_clone().ok(),
        _ => None,
    }
}

/// Read and write timeouts, in milliseconds. **Without this a blocked
/// `tcp_read` waits forever**, which is how a server ends up with threads
/// pinned to peers that went away. A value of 0 or less clears the timeout.
pub fn tcp_set_timeout(
    handle: crate::value::FolInt,
    millis: crate::value::FolInt,
) -> crate::value::FolInt {
    let timeout = if millis > 0 {
        Some(std::time::Duration::from_millis(millis as u64))
    } else {
        None
    };
    let Some(stream) = clone_stream(handle) else {
        return -1;
    };
    if stream.set_read_timeout(timeout).is_err() || stream.set_write_timeout(timeout).is_err() {
        return -1;
    }
    0
}

pub fn tcp_set_nodelay(
    handle: crate::value::FolInt,
    enabled: crate::value::FolBool,
) -> crate::value::FolInt {
    let Some(stream) = clone_stream(handle) else {
        return -1;
    };
    stream.set_nodelay(enabled).map_or(-1, |()| 0)
}

/// A non-blocking read: empty when nothing has arrived YET, which is not the
/// same as the peer closing. `tcp_read` cannot distinguish those either, so a
/// poll loop should pair this with a liveness check of its own.
pub fn tcp_try_read(handle: crate::value::FolInt) -> FolStr {
    use std::io::Read;
    let Some(mut stream) = clone_stream(handle) else {
        return FolStr::new(String::new());
    };
    if stream.set_nonblocking(true).is_err() {
        return FolStr::new(String::new());
    }
    let mut buffer = vec![0u8; 65536];
    let read = stream.read(&mut buffer);
    let _ = stream.set_nonblocking(false);
    match read {
        Ok(0) | Err(_) => FolStr::new(String::new()),
        Ok(count) => FolStr::new(String::from_utf8_lossy(&buffer[..count]).into_owned()),
    }
}

pub fn tcp_peer_addr(handle: crate::value::FolInt) -> FolStr {
    clone_stream(handle)
        .and_then(|stream| stream.peer_addr().ok())
        .map_or_else(
            || FolStr::new(String::new()),
            |addr| FolStr::new(addr.to_string()),
        )
}

/// Half-close: 0 stops reading, 1 stops writing, 2 both. Distinct from
/// `tcp_close`, which releases the handle — shutting down the write side is how
/// a peer is told "no more data" while the read side stays open for its reply.
pub fn tcp_shutdown(
    handle: crate::value::FolInt,
    how: crate::value::FolInt,
) -> crate::value::FolInt {
    let direction = match how {
        0 => std::net::Shutdown::Read,
        1 => std::net::Shutdown::Write,
        _ => std::net::Shutdown::Both,
    };
    clone_stream(handle).map_or(-1, |stream| stream.shutdown(direction).map_or(-1, |()| 0))
}

pub fn udp_bind(address: FolStr) -> crate::value::FolInt {
    match std::net::UdpSocket::bind(address.as_str()) {
        Ok(socket) => register_socket(SocketSlot::Datagram(socket)),
        Err(error) => {
            note_os_error(&error);
            -1
        }
    }
}

pub fn udp_send_to(
    handle: crate::value::FolInt,
    address: FolStr,
    payload: FolStr,
) -> crate::value::FolInt {
    let Some(socket) = clone_datagram(handle) else {
        return -1;
    };
    socket
        .send_to(payload.as_str().as_bytes(), address.as_str())
        .map_or(-1, |sent| sent as crate::value::FolInt)
}

/// Blocks for one datagram, returning `[payload, sender]`. Datagrams arrive
/// whole or not at all, so unlike a stream there is no partial-read case.
pub fn udp_recv_from(handle: crate::value::FolInt) -> FolVec<FolStr> {
    let empty = || FolVec::from_items(vec![FolStr::new(String::new()); 2]);
    let Some(socket) = clone_datagram(handle) else {
        return empty();
    };
    let mut buffer = vec![0u8; 65536];
    match socket.recv_from(&mut buffer) {
        Ok((count, from)) => FolVec::from_items(vec![
            FolStr::new(String::from_utf8_lossy(&buffer[..count]).into_owned()),
            FolStr::new(from.to_string()),
        ]),
        Err(_) => empty(),
    }
}

/// Every address a host name resolves to. The port is stripped: callers want
/// addresses, and `to_socket_addrs` requires one to be present.
pub fn dns_resolve(host: FolStr) -> FolVec<FolStr> {
    use std::net::ToSocketAddrs;
    let query = format!("{}:0", host.as_str());
    match query.to_socket_addrs() {
        Ok(addresses) => FolVec::from_items(
            addresses
                .map(|addr| FolStr::new(addr.ip().to_string()))
                .collect(),
        ),
        Err(error) => {
            note_os_error(&error);
            FolVec::from_items(Vec::new())
        }
    }
}

/// The address a listener actually bound, which is how a caller learns the port
/// after binding to `:0`.
pub fn tcp_local_addr(handle: crate::value::FolInt) -> FolStr {
    let guard = sockets().lock().unwrap_or_else(|error| error.into_inner());
    let rendered = match guard.get(&handle) {
        Some(SocketSlot::Listener(listener)) => listener.local_addr().ok().map(|a| a.to_string()),
        Some(SocketSlot::Stream(stream)) => stream.local_addr().ok().map(|a| a.to_string()),
        Some(SocketSlot::Datagram(socket)) => socket.local_addr().ok().map(|a| a.to_string()),
        None => None,
    };
    FolStr::new(rendered.unwrap_or_default())
}

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
        Err(error) => {
            note_os_error(&error);
            FolStr::new(String::new())
        }
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
    report_unit(std::fs::create_dir_all(path.as_str()))
}

pub fn remove_file(path: FolStr) -> crate::value::FolInt {
    report_unit(std::fs::remove_file(path.as_str()))
}

pub fn rename_file(from: FolStr, to: FolStr) -> crate::value::FolInt {
    report_unit(std::fs::rename(from.as_str(), to.as_str()))
}

pub fn copy_file(from: FolStr, to: FolStr) -> crate::value::FolInt {
    report_unit(std::fs::copy(from.as_str(), to.as_str()).map(|_| ()))
}

pub fn append_file(path: FolStr, contents: FolStr) -> crate::value::FolInt {
    use std::io::Write;
    report_unit(
        std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(path.as_str())
            .and_then(|mut file| file.write_all(contents.as_str().as_bytes())),
    )
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

#[cfg(test)]
mod phase_i_tests {
    use super::*;

    // The published SipHash-2-4 vectors: key = bytes 00..0f, input = the first
    // N bytes of 00,01,02,... Matching these is what makes the implementation
    // SipHash rather than merely a deterministic mixing function.
    const REFERENCE_KEY_LOW: u64 = 0x0706_0504_0302_0100;
    const REFERENCE_KEY_HIGH: u64 = 0x0f0e_0d0c_0b0a_0908;
    const REFERENCE_VECTORS: [u64; 9] = [
        0x726f_db47_dd0e_0e31,
        0x74f8_39c5_93dc_67fd,
        0x0d6c_8009_d9a9_4f5a,
        0x8567_6696_d7fb_7e2d,
        0xcf27_94e0_2771_87b7,
        0x1876_5564_cd99_a68d,
        0xcbc9_466e_58fe_e3ce,
        0xab02_00f5_8b01_d137,
        0x93f5_f579_9a93_2462,
    ];

    #[test]
    fn siphash24_matches_the_published_reference_vectors() {
        for (length, expected) in REFERENCE_VECTORS.iter().enumerate() {
            let input: Vec<u8> = (0..length as u8).collect();
            assert_eq!(
                siphash24(REFERENCE_KEY_LOW, REFERENCE_KEY_HIGH, &input),
                *expected,
                "siphash-2-4 vector for input length {length}"
            );
        }
    }

    #[test]
    fn hash_bytes_is_stable_and_length_sensitive() {
        assert_eq!(
            hash_bytes(FolStr::new("fol")),
            hash_bytes(FolStr::new("fol"))
        );
        assert_ne!(
            hash_bytes(FolStr::new("ab")),
            hash_bytes(FolStr::new("ab\0"))
        );
        assert_ne!(hash_bytes(FolStr::new("")), hash_bytes(FolStr::new("\0")));
    }

    // One flipped bit should move roughly half the output bits; a hash that
    // fails this would still be "deterministic" but would cluster badly.
    #[test]
    fn hash_bytes_avalanches_on_a_single_bit_flip() {
        let base = hash_bytes(FolStr::new("avalanche")) as u64;
        let flipped = hash_bytes(FolStr::new("avalanchd")) as u64;
        let moved = (base ^ flipped).count_ones();
        assert!(
            (16..=48).contains(&moved),
            "expected roughly half of 64 bits to move, got {moved}"
        );
    }

    #[test]
    fn bytes_equal_ct_agrees_with_ordinary_equality() {
        assert!(bytes_equal_ct(FolStr::new("secret"), FolStr::new("secret")));
        assert!(bytes_equal_ct(FolStr::new(""), FolStr::new("")));
        assert!(!bytes_equal_ct(
            FolStr::new("secret"),
            FolStr::new("secrxt")
        ));
        // Differs only in the last byte: the case an early-exit compare leaks.
        assert!(!bytes_equal_ct(
            FolStr::new("secret"),
            FolStr::new("secreT")
        ));
        assert!(!bytes_equal_ct(FolStr::new("secret"), FolStr::new("secre")));
        assert!(!bytes_equal_ct(
            FolStr::new("secret"),
            FolStr::new("secrets")
        ));
    }

    #[test]
    fn backtrace_names_the_capturing_frame() {
        let captured = backtrace();
        assert!(!captured.as_str().is_empty());
    }
}

#[cfg(test)]
mod stream_and_text_tests {
    use super::*;

    fn bytes(values: &[i64]) -> FolVec<crate::value::FolInt> {
        FolVec::from_items(values.to_vec())
    }

    #[test]
    fn str_from_bytes_round_trips_multibyte_text() {
        let text = "héllo wörld";
        let encoded = str_bytes(FolStr::new(text));
        assert_eq!(encoded.as_slice().len(), text.len());
        assert_eq!(str_from_bytes(encoded).as_str(), text);
    }

    #[test]
    fn str_from_bytes_refuses_invalid_rather_than_substituting() {
        // A lone continuation byte, and a value that is not a byte at all.
        assert_eq!(str_from_bytes(bytes(&[104, 233, 108])).as_str(), "");
        assert_eq!(str_from_bytes(bytes(&[104, 300])).as_str(), "");
        assert!(!bytes_valid_utf8(bytes(&[104, 233, 108])));
        assert!(!bytes_valid_utf8(bytes(&[104, 300])));
        assert!(bytes_valid_utf8(bytes(&[104, 105])));
    }

    // The property a chunked reader depends on: splitting anywhere and
    // decoding the complete prefix each time must reassemble the original.
    #[test]
    fn utf8_prefix_len_makes_every_split_point_lossless() {
        let text = "héllo wörld ünïcode";
        let all = str_bytes(FolStr::new(text));
        let raw: Vec<i64> = all.as_slice().to_vec();
        for chunk in 1..=raw.len() {
            let mut carry: Vec<i64> = Vec::new();
            let mut rebuilt = String::new();
            for window in raw.chunks(chunk) {
                carry.extend_from_slice(window);
                let ready = utf8_prefix_len(bytes(&carry)) as usize;
                rebuilt.push_str(str_from_bytes(bytes(&carry[..ready])).as_str());
                carry = carry[ready..].to_vec();
            }
            assert_eq!(rebuilt, text, "chunk size {chunk} lost data");
            assert!(carry.is_empty(), "chunk size {chunk} left a partial tail");
        }
    }

    #[test]
    fn utf8_prefix_len_separates_incomplete_from_malformed() {
        // 0xC3 opens a two-byte sequence, so only "h" is complete.
        assert_eq!(utf8_prefix_len(bytes(&[104, 0xC3])), 1);
        // 0xFF can never begin a sequence.
        assert_eq!(utf8_prefix_len(bytes(&[0xFF, 104])), 0);
        assert_eq!(utf8_prefix_len(bytes(&[104, 105])), 2);
    }

    #[test]
    fn widths_follow_terminal_columns_not_bytes_or_codepoints() {
        assert_eq!(str_width(FolStr::new("hello")), 5);
        // 6 bytes, 2 codepoints, 4 columns.
        assert_eq!(str_width(FolStr::new("日本")), 4);
        assert_eq!(str_width(FolStr::new("👍")), 2);
        // A combining accent adds no column.
        assert_eq!(str_width(FolStr::new("e\u{0301}")), 1);
        assert_eq!(str_width(FolStr::new("héllo")), 5);
        assert_eq!(chr_width('日'), 2);
        assert_eq!(chr_width('a'), 1);
        assert_eq!(chr_width('\u{0301}'), 0);
        // Controls occupy nothing.
        assert_eq!(chr_width('\n'), 0);
    }

    #[test]
    fn flt_to_str_exact_round_trips_where_fixed_decimals_do_not() {
        for value in [0.1_f64, 1.0 / 3.0, 1e-7, 12345.6789, -0.0] {
            let rendered = flt_to_str_exact(value);
            let parsed: f64 = rendered.as_str().parse().expect("should reparse");
            assert_eq!(
                parsed.to_bits(),
                value.to_bits(),
                "{rendered} did not round-trip"
            );
        }
        assert_eq!(flt_to_str_exact(0.1).as_str(), "0.1");
        assert_eq!(flt_to_str_exact(f64::INFINITY).as_str(), "inf");
        assert!(flt_to_str_exact(f64::NAN).as_str() == "nan");
    }

    #[test]
    fn file_handles_stream_seek_and_close() {
        let mut path = std::env::temp_dir();
        path.push(format!("fol_stream_probe_{}.bin", std::process::id()));
        let path_str = FolStr::new(path.to_string_lossy().into_owned());

        let writer = file_open(path_str.clone(), 1);
        assert!(writer > 0);
        let payload = str_bytes(FolStr::new("héllo"));
        assert_eq!(
            file_write(writer, payload.clone()),
            payload.as_slice().len() as i64
        );
        assert_eq!(file_flush(writer), 0);
        assert_eq!(file_close(writer), 0);
        assert_eq!(file_close(writer), -1, "a handle closes once");

        let reader = file_open(path_str.clone(), 0);
        assert_eq!(file_read(reader, 1).as_slice(), &[104]);
        assert_eq!(file_seek(reader, 0, 0), 0);
        assert_eq!(file_seek(reader, -1, 2), 5);
        assert_eq!(file_seek(reader, 0, 9), -1, "unknown whence is refused");
        assert_eq!(file_close(reader), 0);

        assert_eq!(file_open(path_str, 9), -1, "unknown mode is refused");
        assert_eq!(file_read(999_999, 4).as_slice().len(), 0);
        std::fs::remove_file(&path).ok();
    }
}
