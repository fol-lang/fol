//! Hosted runtime tier surface, including console-facing formatting hooks.

use crate::core::RuntimeTier;
use std::sync::{Mutex, OnceLock};
use std::sync::{mpsc, Arc};
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread::JoinHandle;

pub use crate::abi::{check_recoverable, recoverable_succeeded, FolRecover};
pub use crate::aggregate::{
    render_echo, render_entry, render_entry_debug, render_record, render_record_debug,
    FolEchoFormat, FolEntry, FolNamedValue, FolRecord,
};
pub use crate::builtins::{len, pow, pow_float, FolLength};
pub use crate::containers::{
    index_array, index_seq, index_vec, lookup_map, render_array, render_map, render_seq,
    render_set, render_vec, slice_seq, slice_vec, FolArray,
};
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

pub fn join_all_tasks() {
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
            handle.join().expect("spawned FOL task panicked");
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
    pub fn await_value(&self) -> T {
        self.receiver
            .lock()
            .unwrap_or_else(|error| error.into_inner())
            .take()
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
    receiver: Arc<Mutex<mpsc::Receiver<T>>>,
    receiver_closed: Arc<AtomicBool>,
}

impl<T> Default for FolChannel<T> {
    fn default() -> Self {
        let (sender, receiver) = mpsc::channel();
        Self {
            sender: Mutex::new(Some(sender)),
            receiver: Arc::new(Mutex::new(receiver)),
            receiver_closed: Arc::new(AtomicBool::new(false)),
        }
    }
}

impl<T> Clone for FolChannel<T> {
    fn clone(&self) -> Self {
        let sender = self
            .sender
            .lock()
            .unwrap_or_else(|error| error.into_inner())
            .as_ref()
            .cloned();
        Self {
            sender: Mutex::new(sender),
            receiver: self.receiver.clone(),
            receiver_closed: self.receiver_closed.clone(),
        }
    }
}

impl<T> FolChannel<T> {
    pub fn sender(&self) -> FolSender<T> {
        let sender = self
            .sender
            .lock()
            .unwrap_or_else(|error| error.into_inner())
            .as_ref()
            .expect("channel transmitter is no longer available")
            .clone();
        FolSender(sender)
    }

    pub fn send(&self, value: T) {
        self.sender().send(value);
    }

    fn close_local_sender(&self) {
        self.sender
            .lock()
            .unwrap_or_else(|error| error.into_inner())
            .take();
    }

    pub fn receive(&self) -> T {
        self.close_local_sender();
        self.receiver
            .lock()
            .unwrap_or_else(|error| error.into_inner())
            .recv()
            .expect("receive from a closed channel")
    }

    pub fn receive_optional(&self) -> FolOption<T> {
        self.close_local_sender();
        self.receiver
            .lock()
            .unwrap_or_else(|error| error.into_inner())
            .recv()
            .ok()
            .into()
    }

    pub fn try_receive(&self) -> FolOption<T> {
        self.close_local_sender();
        match self
            .receiver
            .lock()
            .unwrap_or_else(|error| error.into_inner())
            .try_recv()
        {
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
    pub fn send(&self, value: T) {
        self.0.send(value).expect("send to a closed channel");
    }
}

#[derive(Debug)]
pub struct FolMutex<T> {
    value: Arc<Mutex<T>>,
    gate: Arc<AtomicBool>,
    owns_gate: AtomicBool,
}

impl<T: Default> Default for FolMutex<T> {
    fn default() -> Self {
        Self {
            value: Arc::new(Mutex::new(T::default())),
            gate: Arc::new(AtomicBool::new(false)),
            owns_gate: AtomicBool::new(false),
        }
    }
}

impl<T> Clone for FolMutex<T> {
    fn clone(&self) -> Self {
        Self {
            value: self.value.clone(),
            gate: self.gate.clone(),
            owns_gate: AtomicBool::new(false),
        }
    }
}

impl<T> FolMutex<T> {
    pub fn from_value(value: T) -> Self {
        Self {
            value: Arc::new(Mutex::new(value)),
            gate: Arc::new(AtomicBool::new(false)),
            owns_gate: AtomicBool::new(false),
        }
    }

    pub fn lock(&self) {
        if self.owns_gate.load(Ordering::Acquire) {
            return;
        }
        while self
            .gate
            .compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire)
            .is_err()
        {
            std::thread::yield_now();
        }
        self.owns_gate.store(true, Ordering::Release);
    }

    pub fn unlock(&self) {
        if self.owns_gate.swap(false, Ordering::AcqRel) {
            self.gate.store(false, Ordering::Release);
        }
    }

    pub fn with<R>(&self, read: impl FnOnce(&T) -> R) -> R {
        let value = self
            .value
            .lock()
            .unwrap_or_else(|error| error.into_inner());
        read(&value)
    }

    pub fn with_mut<R>(&self, write: impl FnOnce(&mut T) -> R) -> R {
        let mut value = self
            .value
            .lock()
            .unwrap_or_else(|error| error.into_inner());
        write(&mut value)
    }
}

impl<T> Drop for FolMutex<T> {
    fn drop(&mut self) {
        self.unlock();
    }
}

pub fn echo<T: FolEchoFormat>(value: T) -> T {
    println!("{}", value.fol_echo_format());
    value
}

pub const FOL_EXIT_SUCCESS: i32 = 0;
pub const FOL_EXIT_FAILURE: i32 = 1;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FolProcessOutcome {
    exit_code: i32,
    message: Option<String>,
}

impl FolProcessOutcome {
    pub fn new(exit_code: i32, message: Option<String>) -> Self {
        Self { exit_code, message }
    }

    pub fn success() -> Self {
        Self::new(FOL_EXIT_SUCCESS, None)
    }

    pub fn failure(message: impl Into<String>) -> Self {
        Self::new(FOL_EXIT_FAILURE, Some(message.into()))
    }

    pub fn exit_code(&self) -> i32 {
        self.exit_code
    }

    pub fn message(&self) -> Option<&str> {
        self.message.as_deref()
    }

    pub fn is_success(&self) -> bool {
        self.exit_code == FOL_EXIT_SUCCESS
    }

    pub fn is_failure(&self) -> bool {
        !self.is_success()
    }
}

pub fn failure_outcome_from_error<E: FolEchoFormat>(error: E) -> FolProcessOutcome {
    FolProcessOutcome::failure(error.fol_echo_format())
}

pub fn printable_outcome_message(outcome: &FolProcessOutcome) -> Option<&str> {
    outcome.message()
}

pub fn outcome_from_recoverable<T, E: FolEchoFormat>(value: FolRecover<T, E>) -> FolProcessOutcome {
    match value {
        FolRecover::Ok(_) => FolProcessOutcome::success(),
        FolRecover::Err(error) => failure_outcome_from_error(error),
    }
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

#[cfg(test)]
mod tests {
    use super::*;

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
        assert!(HAS_HEAP);
        assert!(HAS_OS);
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

    #[test]
    fn recoverable_entry_results_map_to_minimal_process_outcomes() {
        let success = outcome_from_recoverable(FolRecover::<i64, FolStr>::ok(7));
        let failure =
            outcome_from_recoverable(FolRecover::<i64, FolStr>::err(FolStr::from("bad-input")));

        assert_eq!(success, FolProcessOutcome::success());
        assert!(success.is_success());
        assert_eq!(success.message(), None);

        assert_eq!(failure, FolProcessOutcome::failure("bad-input"));
        assert!(failure.is_failure());
        assert_eq!(failure.message(), Some("bad-input"));
    }

    #[test]
    fn failure_helpers_keep_printable_messages_stable() {
        let failure = failure_outcome_from_error(FolStr::from("broken"));

        assert_eq!(failure, FolProcessOutcome::failure("broken"));
        assert_eq!(printable_outcome_message(&failure), Some("broken"));
        assert_eq!(
            printable_outcome_message(&FolProcessOutcome::success()),
            None
        );
    }

    #[test]
    fn exit_code_constants_freeze_minimal_v1_process_policy() {
        assert_eq!(FOL_EXIT_SUCCESS, 0);
        assert_eq!(FOL_EXIT_FAILURE, 1);
        assert_eq!(FolProcessOutcome::success().exit_code(), FOL_EXIT_SUCCESS);
        assert_eq!(
            FolProcessOutcome::failure("broken").exit_code(),
            FOL_EXIT_FAILURE
        );
    }

    #[test]
    fn top_level_success_and_failure_messages_stay_backend_ready() {
        let success = outcome_from_recoverable(FolRecover::<i64, FolStr>::ok(9));
        let failure =
            outcome_from_recoverable(FolRecover::<i64, FolStr>::err(FolStr::from("fatal")));

        assert!(success.is_success());
        assert_eq!(success.exit_code(), FOL_EXIT_SUCCESS);
        assert_eq!(printable_outcome_message(&success), None);

        assert!(failure.is_failure());
        assert_eq!(failure.exit_code(), FOL_EXIT_FAILURE);
        assert_eq!(printable_outcome_message(&failure), Some("fatal"));
    }
}
