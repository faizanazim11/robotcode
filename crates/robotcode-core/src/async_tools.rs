//! Cancellation tokens and async mutex helpers.
//!
//! Port of the Python `robotcode.core.async_tools` module.

use std::future::Future;
use std::pin::Pin;
use std::task::{Context, Poll};

use tokio::sync::{Mutex, MutexGuard};
use tokio_util::sync::CancellationToken;

pub use tokio_util::sync::CancellationToken as Token;

/// A guard that cancels the token when dropped.
pub struct DropGuard(#[allow(dead_code)] tokio_util::sync::DropGuard);

impl DropGuard {
    pub fn disarm(self) {
        // The inner DropGuard::disarm() consumes self and prevents cancellation.
        // We recreate the behaviour by just dropping without cancelling.
        std::mem::forget(self);
    }
}

/// Create a new cancellation token together with a drop guard that cancels it
/// automatically when it goes out of scope.
pub fn new_cancellable() -> (CancellationToken, tokio_util::sync::DropGuard) {
    let token = CancellationToken::new();
    let guard = token.clone().drop_guard();
    (token, guard)
}

/// Run `fut` and cancel it when the token is cancelled.
///
/// Returns `None` if the token was cancelled before the future completed,
/// or `Some(value)` if it finished.
pub async fn with_cancellation<F, T>(token: &CancellationToken, fut: F) -> Option<T>
where
    F: Future<Output = T>,
{
    tokio::select! {
        biased;
        _ = token.cancelled() => None,
        v = fut => Some(v),
    }
}

/// A simple async-aware mutex that wraps [`tokio::sync::Mutex`].
///
/// Matches the `AsyncMutex` helper pattern used in the Python code.
pub struct AsyncMutex<T>(Mutex<T>);

impl<T> AsyncMutex<T> {
    pub fn new(value: T) -> Self {
        Self(Mutex::new(value))
    }

    pub async fn lock(&self) -> MutexGuard<'_, T> {
        self.0.lock().await
    }

    pub fn try_lock(&self) -> Result<MutexGuard<'_, T>, tokio::sync::TryLockError> {
        self.0.try_lock()
    }
}

impl<T: std::fmt::Debug> std::fmt::Debug for AsyncMutex<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_tuple("AsyncMutex").field(&self.0).finish()
    }
}

/// A future that is always immediately ready.
///
/// Useful as a placeholder / stub in tests.
pub struct ReadyFuture<T>(Option<T>);

impl<T: Unpin> Future for ReadyFuture<T> {
    type Output = T;
    fn poll(mut self: Pin<&mut Self>, _cx: &mut Context<'_>) -> Poll<T> {
        Poll::Ready(self.0.take().expect("ReadyFuture polled after completion"))
    }
}

/// Create a future that resolves immediately with `value`.
pub fn ready<T>(value: T) -> ReadyFuture<T> {
    ReadyFuture(Some(value))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_cancellation_token_cancelled() {
        let token = CancellationToken::new();
        token.cancel();
        let result = with_cancellation(&token, async { 42u32 }).await;
        assert!(result.is_none());
    }

    #[tokio::test]
    async fn test_cancellation_token_not_cancelled() {
        let token = CancellationToken::new();
        let result = with_cancellation(&token, async { 42u32 }).await;
        assert_eq!(result, Some(42));
    }

    #[tokio::test]
    async fn test_async_mutex() {
        let m = AsyncMutex::new(0u32);
        {
            let mut g = m.lock().await;
            *g = 42;
        }
        assert_eq!(*m.lock().await, 42);
    }
}
