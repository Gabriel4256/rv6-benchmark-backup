use core::{cell::UnsafeCell, marker::PhantomData, pin::Pin};

use super::{Guard, Lock, RawLock};

/// `RemoteLock<'s, R, U, T>`, such as `RemoteLock<'s, RawSpinlock, U, T>`.
///
/// Similar to `Lock<R, T>`, but uses a shared raw lock.
/// * At creation, a `RemoteLock` borrows a raw lock from a `Lock`.
/// * To access its inner data, you must use that `Lock`'s guard.
///
/// In this way, we can make a single raw lock be shared by a `Lock` and multiple `RemoteLock`s.
/// * See the [lock](`super`) module documentation for details.
///
/// # Note
///
/// To dereference the inner data, you must use `RemoteLock::get_pin_mut_unchecked`
/// or `RemoteLock::get_mut_unchecked`.
pub struct RemoteLock<'s, R: RawLock, U, T> {
    data: UnsafeCell<T>,
    _marker: PhantomData<&'s Lock<R, U>>,
}

unsafe impl<'s, R: RawLock, U: Send, T: Send> Sync for RemoteLock<'s, R, U, T> {}

impl<'s, R: RawLock, U, T> RemoteLock<'s, R, U, T> {
    /// Returns a `RemoteLock` that protects `data` using the given `lock`.
    /// `lock` could be any [`Lock`], such as [Spinlock](super::Spinlock), [Sleepablelock](super::Sleepablelock), or [Sleeplock](super::Sleeplock).
    ///
    /// # Examples
    ///
    /// ```rust,no_run
    /// let spinlock: Spinlock<usize> = Spinlock::new("spinlock", 10);
    /// let spinlock_remote: RemoteLock<'_, RawSpinlock, usize, isize> = RemoteLock::new(&spinlock, -20);
    /// ```
    pub const fn new(_lock: &'s Lock<R, U>, data: T) -> Self {
        Self {
            data: UnsafeCell::new(data),
            _marker: PhantomData,
        }
    }

    /// Returns a raw pointer to the inner data.
    /// The returned pointer is valid until this `RemoteLock` is moved or dropped.
    /// The caller must ensure that accessing the pointer does not incur race.
    /// Also, if `T: !Unpin`, the caller must not move the data using the pointer.
    pub fn get_mut_raw(&self) -> *mut T {
        self.data.get()
    }

    /// Returns a pinned mutable reference to the inner data.
    ///
    /// # Safety
    ///
    /// The provided `guard` must be from the `Lock` that this `RemoteLock` borrowed from.
    /// You may want to wrap this function with a safe function that uses branded types.
    pub unsafe fn get_pin_mut_unchecked<'t>(
        &'t self,
        _guard: &'t mut Guard<'_, R, U>,
    ) -> Pin<&'t mut T> {
        unsafe { Pin::new_unchecked(&mut *self.data.get()) }
    }
}

impl<'s, R: RawLock, U, T: Unpin> RemoteLock<'s, R, U, T> {
    /// Returns a mutable reference to the inner data.
    ///
    /// # Safety
    ///
    /// The provided `guard` must be from the `Lock` that this `RemoteLock` borrowed from.
    /// You may want to wrap this function with a safe function that uses branded types.
    pub unsafe fn get_mut_unchecked<'t>(&'t self, guard: &'t mut Guard<'_, R, U>) -> &'t mut T {
        unsafe { self.get_pin_mut_unchecked(guard) }.get_mut()
    }
}