//! Hosted runtime tier surface, including console-facing formatting hooks.

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
pub use crate::builtins::{len, pow, pow_float, FolLength};
pub use crate::containers::{
    index_array, index_seq, index_set, index_vec, lookup_map, render_array, render_map, render_seq,
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
    receiver: Mutex<mpsc::Receiver<T>>,
    receiver_closed: AtomicBool,
}

impl<T> Default for FolChannel<T> {
    fn default() -> Self {
        let (sender, receiver) = mpsc::channel();
        Self {
            sender: Mutex::new(Some(sender)),
            receiver: Mutex::new(receiver),
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
    pub fn send(&self, value: T) -> Result<(), T> {
        self.0.send(value).map_err(|error| error.0)
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

        let left = channel.receive();
        let right = channel.receive();
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

        assert_eq!(channel.receive(), 19);
        assert!(channel.acquire_sender().is_none());

        sender.send(23).expect("pre-acquired sender remains valid");
        assert_eq!(channel.receive(), 23);
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
