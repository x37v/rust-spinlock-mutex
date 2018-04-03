#![feature(optin_builtin_traits)]

use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use std::cell::UnsafeCell;
use std::ops::{Deref, DerefMut};
use std::fmt;

/// An atomic number based lock that makes no system calls and busy waits instead of locking.
/// modeled after std::sync::Mutex and std::sync::MutexGuard
/// the examples and documentation are just slight edits of the examples and documentation from
/// those.
pub struct Mutex<T: ?Sized> {
    shared_value: Arc<AtomicUsize>,
    next_id: Arc<AtomicUsize>,
    id: usize,
    data: Arc<UnsafeCell<T>>
}

type TryLockResult<MutexGuard> = Result<MutexGuard, usize>;

impl<T> Mutex<T> {
    /// Creates a new spinlock in an unlocked state ready for use.
    ///
    /// # Examples
    ///
    /// ```
    /// use spinlock::Mutex;
    ///
    /// let sl = Mutex::new(1984);
    /// ```
    pub fn new(t: T) -> Self {
        Mutex {
            shared_value: Arc::new(AtomicUsize::new(0)),
            next_id: Arc::new(AtomicUsize::new(2)),
            id: 1,
            data: Arc::new(UnsafeCell::new(t))
        }
    }
}

impl<T: ?Sized> Mutex<T> {
    /// Locks a spinlock, busy waiting until the exclusive access is available.
    ///
    /// The function will spin in the local thread until it is available to acquire
    /// the lock. Upon returning, the thread is the only thread with the lock held.
    /// An RAII guard is returned to allow scoped unlock of the lock. When the guard
    /// goes out of scope, the spinlock will be unlocked.
    ///
    /// # Panics
    ///
    /// This function will panic a lock has already been acquired with the same clone of the
    /// spinlock.
    ///
    /// # Examples
    ///
    /// ```
    /// use spinlock::Mutex;
    /// use std::thread;
    ///
    /// let sl = Mutex::new(1984);
    /// let cl = sl.clone();
    /// thread::spawn(move ||{
    ///     *cl.lock() = 2084;
    /// }).join().expect("thread::spawn failed");
    /// assert_eq!(*sl.lock(), 2084);
    /// ```
    pub fn lock(&self) -> MutexGuard<T> {
        //spin 
        while self.shared_value.compare_and_swap(0, self.id, Ordering::SeqCst) != self.id {
        }
        MutexGuard {
            lock: self
        }
    }

    /// Attempts to acquire a lock.
    ///
    /// If the lock could not be acquire at this time, then [`Err`] is returned.
    /// Otherwise, an RAII guard is returned. The lock will be unlocked when the
    /// guard is dropped.
    ///
    /// This function does not block.
    /// 
    /// # Panics
    ///
    /// This function will panic a lock has already been acquired with the same clone of the
    /// spinlock.
    ///
    /// ```
    /// use spinlock::Mutex;
    /// use std::thread;
    ///
    /// let sl = Mutex::new(2084);
    /// let cl = sl.clone();
    ///
    /// thread::spawn(move || {
    ///     let mut lock = cl.try_lock();
    ///     if let Ok(ref mut mutex) = lock {
    ///         **mutex = 10;
    ///     } else {
    ///         println!("try_lock failed");
    ///     }
    /// }).join().expect("thread::spawn failed");
    /// assert_eq!(*sl.lock(), 10);
    /// ```
    pub fn try_lock(&self) -> TryLockResult<MutexGuard<T>> {
        assert_ne!(self.id, self.shared_value.load(Ordering::SeqCst));
        let id = self.shared_value.compare_and_swap(0, self.id, Ordering::SeqCst);
        if self.id == self.shared_value.load(Ordering::SeqCst) {
            Ok(MutexGuard { lock: self })
        } else {
            Err(id)
        }
    }
}

impl<T: ?Sized + Default> Default for Mutex<T> {
    fn default() -> Mutex<T> {
        Mutex::new(Default::default())
    }
}

impl<T> Clone for Mutex<T> {
    fn clone(&self) -> Self {
        let id = self.next_id.fetch_add(1, Ordering::SeqCst);
        Mutex {
            shared_value: self.shared_value.clone(),
            next_id: self.next_id.clone(),
            id: id,
            data: self.data.clone()
        }
    }
}

impl<T: ?Sized + fmt::Debug> fmt::Debug for Mutex<T> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self.try_lock() {
            Ok(guard) => f.debug_struct("Mutex").field("data", &&*guard).finish(),
            Err(id) => {
                f.debug_struct("Mutex").field("locked_by", &id).finish()
            }
        }
    }
}

unsafe impl<T: ?Sized + Send> Send for Mutex<T> {}
unsafe impl<T: ?Sized + Sync> Sync for Mutex<T> {}

/// An RAII implementation of a "scoped lock" of a spinlock. When this structure is dropped (falls out
/// of scope), the lock will be unlocked.
pub struct MutexGuard<'a, T: ?Sized + 'a> {
    lock: &'a Mutex<T>
}

impl<'a, T: ?Sized> Drop for MutexGuard<'a, T> {
    fn drop(&mut self) {
        assert_eq!(self.lock.shared_value.load(Ordering::SeqCst), self.lock.id);
        self.lock.shared_value.store(0, Ordering::SeqCst);
    }
}

impl<'a, T: ?Sized> !Send for MutexGuard<'a, T> {}

impl<'a, T: ?Sized> Deref for MutexGuard<'a, T> {
    type Target = T;

    fn deref(&self) -> &T {
        unsafe { &*self.lock.data.get() }
    }
}

impl<'a, T: ?Sized> DerefMut for MutexGuard<'a, T> {
    fn deref_mut(&mut self) -> &mut T {
        unsafe { &mut *self.lock.data.get() }
    }
}

impl<'a, T: ?Sized + fmt::Debug> fmt::Debug for MutexGuard<'a, T> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.debug_struct("MutexGuard")
            .field("lock", &self.lock)
            .finish()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::thread;
    use std::sync::atomic::{AtomicUsize, Ordering, ATOMIC_USIZE_INIT};

    #[test]
    fn smoke() {
        let m = Mutex::new(());
        drop(m.lock());
        drop(m.lock());
    }

    #[test]
    fn initial_and_clone() {
        let lock = Mutex::new(22);
        assert_eq!(lock.id, 1);
        assert_eq!(lock.next_id.load(Ordering::SeqCst), 2);
        assert_eq!(lock.shared_value.load(Ordering::SeqCst), 0);
        unsafe { assert_eq!(*lock.data.get(), 22); }

        let clone = lock.clone();
        assert_eq!(clone.id, 2);
        assert_eq!(clone.next_id.load(Ordering::SeqCst), 3);
        assert_eq!(clone.shared_value.load(Ordering::SeqCst), 0);
        unsafe { assert_eq!(*lock.data.get(), 22); }

        {
            let mut g = clone.lock();
            assert_eq!(clone.shared_value.load(Ordering::SeqCst), 2);
            assert_eq!(lock.shared_value.load(Ordering::SeqCst), 2);
            assert_eq!(*g, 22);
            *g = 42;
        }
        assert_eq!(lock.shared_value.load(Ordering::SeqCst), 0);
        assert_eq!(clone.shared_value.load(Ordering::SeqCst), 0);
        {
            let g = lock.lock();
            assert_eq!(clone.shared_value.load(Ordering::SeqCst), 1);
            assert_eq!(lock.shared_value.load(Ordering::SeqCst), 1);
            assert_eq!(*g, 42);
        }
        assert_eq!(lock.shared_value.load(Ordering::SeqCst), 0);
        assert_eq!(clone.shared_value.load(Ordering::SeqCst), 0);
    }

    #[test]
    fn try_lock() {
        let lock = Mutex::new(532);
        assert_eq!(lock.id, 1);
        assert_eq!(lock.next_id.load(Ordering::SeqCst), 2);
        assert_eq!(lock.shared_value.load(Ordering::SeqCst), 0);

        let clone = lock.clone();
        {
            let g = lock.try_lock().unwrap();
            assert_eq!(lock.shared_value.load(Ordering::SeqCst), 1);
            assert_eq!(*g, 532);

            {
                let g2 = clone.try_lock(); //should fail
                assert!(g2.is_err());
            }
            assert_eq!(*g, 532);
        }

        {
            let g2 = clone.try_lock(); //should pass
            assert!(g2.is_ok());
        }
    }

    #[test]
    fn threaded() {
        let lock = Mutex::new(1984);
        let clone = lock.clone();
        let child = thread::spawn(move || {
            let mut g = clone.lock();
            assert_eq!(*g, 1984);
            *g = 24;
        });
        if let Err(e) = child.join() {
            panic!(e);
        }

        let clone = lock.clone();
        let child = thread::spawn(move || {
            let g = clone.lock();
            assert_eq!(*g, 24);
        });
        if let Err(e) = child.join() {
            panic!(e);
        }
    }

    // copied/edited from crossbeam's arc_cell test
    #[test]
    fn drops() {
        static DROPS: AtomicUsize = ATOMIC_USIZE_INIT;

        struct Foo;

        impl Drop for Foo {
            fn drop(&mut self) {
                DROPS.fetch_add(1, Ordering::SeqCst);
            }
        }

        assert_eq!(DROPS.load(Ordering::SeqCst), 0);
        let l = Mutex::new(Foo);
        let c = l.clone();
        drop(l);
        assert_eq!(DROPS.load(Ordering::SeqCst), 0);
        drop(c);
        assert_eq!(DROPS.load(Ordering::SeqCst), 1);
    }
}
