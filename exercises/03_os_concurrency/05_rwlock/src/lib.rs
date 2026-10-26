//! # Read-Write Lock (Writer-Priority)
//!
//! In this exercise, you will implement a **writer-priority** read-write lock from scratch using atomics.
//! Multiple readers may hold the lock concurrently; a writer holds it exclusively.
//!
//! **Note:** Rust's standard library already provides [`std::sync::RwLock`]. This exercise implements
//! a minimal version for learning the protocol and policy without using the standard one.
//!
//! ## Common policies for read-write locks
//! Different implementations can give different **priority** when both readers and writers are waiting:
//!
//! - **Reader-priority (读者优先)**: New readers are allowed to enter while a writer is waiting, so writers
//!   may be starved if readers keep arriving.
//! - **Writer-priority (写者优先)**: Once a writer is waiting, no new readers are admitted until that writer
//!   has run; this exercise implements this policy.
//! - **Read-write fair (读写公平)**: Requests are served in a fair order (e.g. FIFO or round-robin), so
//!   neither readers nor writers are systematically starved.
//!
//! ## Key Concepts
//! - **Readers**: share access; many threads can hold a read lock at once.
//! - **Writer**: exclusive access; only one writer, and no readers while the writer holds the lock.
//! - **Writer-priority (this implementation)**: when at least one writer is waiting, new readers block
//!   until the writer runs.
//!
//! ## State (single atomic)
//! We use one `AtomicU32`: 15 reader-count bits, 15 waiting-writer-count bits,
//! and two flags for a writer holding / writers waiting. Counting every waiter
//! preserves writer priority even when several writers are queued.
//! All logic is implemented with compare_exchange and load/store; no use of `std::sync::RwLock`.

use std::cell::UnsafeCell;
use std::ops::{Deref, DerefMut};
use std::sync::atomic::{AtomicU32, Ordering};

/// Maximum number of concurrent readers (fits in state bits).
const READER_MASK: u32 = (1 << 15) - 1;
const WAITER_INCREMENT: u32 = 1 << 15;
const WAITER_MASK: u32 = READER_MASK << 15;
/// Bit set when a writer holds the lock.
const WRITER_HOLDING: u32 = 1 << 30;
/// Bit set when at least one writer is waiting (writer-priority: block new readers).
const WRITER_WAITING: u32 = 1 << 31;

/// Writer-priority read-write lock. Implemented from scratch; does not use `std::sync::RwLock`.
pub struct RwLock<T> {
    state: AtomicU32,
    data: UnsafeCell<T>,
}

unsafe impl<T: Send> Send for RwLock<T> {}
unsafe impl<T: Send + Sync> Sync for RwLock<T> {}

impl<T> RwLock<T> {
    pub const fn new(data: T) -> Self {
        Self {
            state: AtomicU32::new(0),
            data: UnsafeCell::new(data),
        }
    }

    /// Acquire a read lock. Blocks (spins) until no writer holds and no writer is waiting (writer-priority).
    ///
    /// TODO: Implement read lock acquisition
    /// 1. In a loop, load state (Acquire).
    /// 2. If WRITER_HOLDING or WRITER_WAITING is set, spin_loop and continue (writer-priority: no new readers while writer waits).
    /// 3. If reader count (state & READER_MASK) is already READER_MASK, spin and continue.
    /// 4. Try compare_exchange(s, s + 1, AcqRel, Acquire); on success return RwLockReadGuard { lock: self }.
    pub fn read(&self) -> RwLockReadGuard<'_, T> {
        // TODO
        loop {
            let state = self.state.load(Ordering::Acquire);
            if state & (WRITER_HOLDING | WRITER_WAITING) != 0 || state & READER_MASK == READER_MASK
            {
                core::hint::spin_loop();
                continue;
            }
            if self
                .state
                .compare_exchange_weak(state, state + 1, Ordering::AcqRel, Ordering::Acquire)
                .is_ok()
            {
                return RwLockReadGuard { lock: self };
            }
        }
    }

    /// Acquire the write lock. Blocks until no readers and no other writer.
    ///
    /// TODO: Implement write lock acquisition (writer-priority)
    /// 1. Register a waiting writer with CAS and set WRITER_WAITING to block new readers.
    /// 2. Spin while readers or another writer hold the lock.
    /// 3. With CAS, remove this waiter and set WRITER_HOLDING. Keep WRITER_WAITING
    ///    set until every queued writer has acquired the lock.
    /// 4. Return an RAII write guard on success.
    pub fn write(&self) -> RwLockWriteGuard<'_, T> {
        // TODO
        // Register this writer in the same atomic state before waiting for readers.
        loop {
            let state = self.state.load(Ordering::Acquire);
            if state & WAITER_MASK == WAITER_MASK {
                core::hint::spin_loop();
                continue;
            }
            let waiting = (state + WAITER_INCREMENT) | WRITER_WAITING;
            if self
                .state
                .compare_exchange_weak(state, waiting, Ordering::AcqRel, Ordering::Acquire)
                .is_ok()
            {
                break;
            }
        }
        loop {
            let state = self.state.load(Ordering::Acquire);
            if state & (READER_MASK | WRITER_HOLDING) == 0 {
                let mut acquired = (state - WAITER_INCREMENT) | WRITER_HOLDING;
                if acquired & WAITER_MASK == 0 {
                    acquired &= !WRITER_WAITING;
                }
                if self
                    .state
                    .compare_exchange_weak(state, acquired, Ordering::AcqRel, Ordering::Acquire)
                    .is_ok()
                {
                    return RwLockWriteGuard { lock: self };
                }
            }
            core::hint::spin_loop();
        }
    }
}

/// Guard for a read lock; releases the read lock on drop.
pub struct RwLockReadGuard<'a, T> {
    lock: &'a RwLock<T>,
}

// TODO: Implement Deref for RwLockReadGuard
// Return shared reference to data: unsafe { &*self.lock.data.get() }
impl<T> Deref for RwLockReadGuard<'_, T> {
    type Target = T;

    fn deref(&self) -> &T {
        // SAFETY: A read guard keeps writers out; all concurrent guards only read.
        unsafe { &*self.lock.data.get() }
    }
}

// TODO: Implement Drop for RwLockReadGuard
// Decrement reader count: self.lock.state.fetch_sub(1, Ordering::Release)
impl<T> Drop for RwLockReadGuard<'_, T> {
    fn drop(&mut self) {
        self.lock.state.fetch_sub(1, Ordering::Release);
    }
}

/// Guard for a write lock; releases the write lock on drop.
pub struct RwLockWriteGuard<'a, T> {
    lock: &'a RwLock<T>,
}

// TODO: Implement Deref for RwLockWriteGuard
// Return shared reference: unsafe { &*self.lock.data.get() }
impl<T> Deref for RwLockWriteGuard<'_, T> {
    type Target = T;

    fn deref(&self) -> &T {
        // SAFETY: A write guard owns exclusive access to the protected data.
        unsafe { &*self.lock.data.get() }
    }
}

// TODO: Implement DerefMut for RwLockWriteGuard
// Return mutable reference: unsafe { &mut *self.lock.data.get() }
impl<T> DerefMut for RwLockWriteGuard<'_, T> {
    fn deref_mut(&mut self) -> &mut T {
        // SAFETY: Both the write lock and this guard borrow are exclusive.
        unsafe { &mut *self.lock.data.get() }
    }
}

// TODO: Implement Drop for RwLockWriteGuard
// Clear WRITER_HOLDING with Release, preserving any queued writers.
impl<T> Drop for RwLockWriteGuard<'_, T> {
    fn drop(&mut self) {
        // Keep queued writers and their waiting flag, so new readers cannot barge in.
        self.lock
            .state
            .fetch_and(!WRITER_HOLDING, Ordering::Release);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;
    use std::thread;

    #[test]
    fn test_multiple_readers() {
        let lock = Arc::new(RwLock::new(0u32));
        let mut handles = vec![];
        for _ in 0..10 {
            let l = Arc::clone(&lock);
            handles.push(thread::spawn(move || {
                let g = l.read();
                assert_eq!(*g, 0);
            }));
        }
        for h in handles {
            h.join().unwrap();
        }
    }

    #[test]
    fn test_writer_excludes_readers() {
        let lock = Arc::new(RwLock::new(0u32));
        let lock_w = Arc::clone(&lock);
        let writer = thread::spawn(move || {
            let mut g = lock_w.write();
            *g = 42;
        });
        writer.join().unwrap();
        let g = lock.read();
        assert_eq!(*g, 42);
    }

    #[test]
    fn test_concurrent_reads_after_write() {
        let lock = Arc::new(RwLock::new(Vec::<i32>::new()));
        {
            let mut g = lock.write();
            g.push(1);
            g.push(2);
        }
        let mut handles = vec![];
        for _ in 0..5 {
            let l = Arc::clone(&lock);
            handles.push(thread::spawn(move || {
                let g = l.read();
                assert_eq!(g.len(), 2);
                assert_eq!(&*g, &[1, 2]);
            }));
        }
        for h in handles {
            h.join().unwrap();
        }
    }

    #[test]
    fn test_concurrent_writes_serialized() {
        let lock = Arc::new(RwLock::new(0u64));
        let mut handles = vec![];
        for _ in 0..10 {
            let l = Arc::clone(&lock);
            handles.push(thread::spawn(move || {
                for _ in 0..100 {
                    let mut g = l.write();
                    *g += 1;
                }
            }));
        }
        for h in handles {
            h.join().unwrap();
        }
        assert_eq!(*lock.read(), 1000);
    }
}
