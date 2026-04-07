//! Event / callback system with weak-reference listeners.
//!
//! Port of the Python `robotcode.core.event` module.
//!
//! # Example
//! ```
//! use robotcode_core::event::Event;
//!
//! let mut event: Event<String> = Event::new();
//! let handle = event.subscribe(|msg| println!("got: {}", msg));
//! event.fire(&"hello".to_string());
//! // handle is dropped → listener is automatically removed.
//! drop(handle);
//! event.fire(&"no listeners".to_string());
//! ```

use std::sync::{Arc, Mutex, Weak};

/// A subscription handle.  Dropping this handle removes the listener from the event.
pub struct Subscription<T: ?Sized>(#[allow(dead_code)] Arc<dyn Fn(&T) + Send + Sync + 'static>);

/// Weak reference stored in the event's listener list.
type WeakListener<T> = Weak<dyn Fn(&T) + Send + Sync + 'static>;

/// A broadcast event with multiple weak-reference listeners.
///
/// Listeners are automatically removed when the [`Subscription`] handle is dropped.
pub struct Event<T: ?Sized> {
    listeners: Arc<Mutex<Vec<WeakListener<T>>>>,
}

impl<T: ?Sized> Default for Event<T> {
    fn default() -> Self {
        Self::new()
    }
}

impl<T: ?Sized> Event<T> {
    /// Create a new empty event.
    pub fn new() -> Self {
        Self {
            listeners: Arc::new(Mutex::new(Vec::new())),
        }
    }

    /// Register a listener callback.
    ///
    /// Returns a [`Subscription`] handle.  The listener is active for as long
    /// as the handle is alive.
    pub fn subscribe<F>(&mut self, f: F) -> Subscription<T>
    where
        F: Fn(&T) + Send + Sync + 'static,
    {
        let arc: Arc<dyn Fn(&T) + Send + Sync + 'static> = Arc::new(f);
        let weak = Arc::downgrade(&arc);
        self.listeners.lock().unwrap().push(weak);
        Subscription(arc)
    }

    /// Fire the event, calling all live listeners with a reference to `value`.
    ///
    /// Dead (dropped) listeners are pruned automatically.
    pub fn fire(&self, value: &T) {
        let mut listeners = self.listeners.lock().unwrap();
        listeners.retain(|weak| {
            if let Some(cb) = weak.upgrade() {
                cb(value);
                true
            } else {
                false
            }
        });
    }

    /// Return the number of currently live listeners.
    pub fn listener_count(&self) -> usize {
        let mut listeners = self.listeners.lock().unwrap();
        // Prune dead entries while counting
        listeners.retain(|w| w.strong_count() > 0);
        listeners.len()
    }
}

impl<T: ?Sized + std::fmt::Debug> std::fmt::Debug for Event<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Event")
            .field("listener_count", &self.listener_count())
            .finish()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicU32, Ordering};

    #[test]
    fn test_fire_calls_listeners() {
        let counter = Arc::new(AtomicU32::new(0));
        let c = Arc::clone(&counter);
        let mut event: Event<u32> = Event::new();
        let _handle = event.subscribe(move |v| {
            c.fetch_add(*v, Ordering::SeqCst);
        });
        event.fire(&10);
        event.fire(&5);
        assert_eq!(counter.load(Ordering::SeqCst), 15);
    }

    #[test]
    fn test_listener_removed_on_drop() {
        let mut event: Event<u32> = Event::new();
        let counter = Arc::new(AtomicU32::new(0));
        let c = Arc::clone(&counter);
        let handle = event.subscribe(move |_| {
            c.fetch_add(1, Ordering::SeqCst);
        });
        event.fire(&1);
        assert_eq!(counter.load(Ordering::SeqCst), 1);
        drop(handle);
        event.fire(&1);
        // Listener was dropped → counter should not increase
        assert_eq!(counter.load(Ordering::SeqCst), 1);
    }

    #[test]
    fn test_listener_count() {
        let mut event: Event<u32> = Event::new();
        assert_eq!(event.listener_count(), 0);
        let h1 = event.subscribe(|_| {});
        let h2 = event.subscribe(|_| {});
        assert_eq!(event.listener_count(), 2);
        drop(h1);
        assert_eq!(event.listener_count(), 1);
        drop(h2);
        assert_eq!(event.listener_count(), 0);
    }
}
