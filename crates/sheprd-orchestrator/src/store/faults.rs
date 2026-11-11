//! Deterministic session-write failures, scoped to one test's dispatch thread.
//! Only the Unix-only Herdr 2 dispatch tests arm them.
use std::cell::Cell;
#[cfg(unix)]
use std::{marker::PhantomData, rc::Rc};

thread_local! {
    static REMAINING: Cell<usize> = const { Cell::new(0) };
}

#[cfg(unix)]
pub(crate) struct Failure(PhantomData<Rc<()>>);

#[cfg(unix)]
impl Drop for Failure {
    fn drop(&mut self) {
        REMAINING.set(0);
    }
}

#[cfg(unix)]
pub(crate) fn session_updates(count: usize) -> Failure {
    assert_eq!(REMAINING.replace(count), 0);
    Failure(PhantomData)
}

#[cfg(unix)]
pub(crate) fn remaining() -> usize {
    REMAINING.get()
}

pub(super) fn before_session_update() -> crate::Result<()> {
    let left = REMAINING.get();
    if left == 0 {
        return Ok(());
    }
    REMAINING.set(left - 1);
    Err(rusqlite::Error::SqliteFailure(
        rusqlite::ffi::Error::new(rusqlite::ffi::SQLITE_BUSY),
        Some(format!("injected session write failure {left}")),
    )
    .into())
}
