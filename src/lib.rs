// Copyright 2017 Andrew D. Straw.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

//! Track changes to data and notify listeners.
//!
//! The main API feature is the [`DataTracker`](./struct.DataTracker.html)
//! struct, which takes ownership of a value.
//!
//! To listen for changes to an instance of the `DataTracker` struct, the
//! function
//! [DataTracker::add_listener()](struct.DataTracker.html#method.add_listener)
//! returns a `futures::Stream`. This `Stream` is supplied with previous and new
//! values immediately after each change and integrates with the
//! [tokio](https://tokio.rs) ecosystem.
//!
//! The principle of operation is that
//! [`DataTracker::as_tracked_mut()`](./struct.DataTracker.html#method.as_tracked_mut)
//! returns a mutable [`Modifier`](./struct.Modifier.html). `Modifier` is a RAII
//! scoped guard with two key properties:
//!
//! - It implements the
//!   [`DerefMut`](https://doc.rust-lang.org/std/ops/trait.DerefMut.html) trait
//!   to allow ergonomic access to the underlying data.
//! - It implements the
//!   [`Drop`](https://doc.rust-lang.org/std/ops/trait.Drop.html) trait which
//!   checks if the underlying data was changed and, if so, notifies the
//!   listeners.
//!
//! Futhermore,
//! [`DataTracker::as_ref()`](./struct.DataTracker.html#method.as_ref) returns a
//! (non-mutable) reference to the data for cases when only read-only access to
//! the data is needed.
//!
//! To implement tracking, when [`Modifier`](./struct.Modifier.html) is created,
//! a copy of the original data is made and when `Modifier` is dropped, an
//! equality check is performed. If the original and the new data are not equal,
//! the callbacks are called with references to the old and the new values.
//!
//! ## example
//!
//! ```
//! extern crate futures;
//! extern crate tokio_core;
//! extern crate raii_change_tracker;
//!
//! use futures::{Future, IntoFuture, Stream};
//! use tokio_core::reactor::{Core, Timeout};
//! use std::time::Duration;
//! use std::rc::Rc;
//! use std::cell::RefCell;
//!
//! use raii_change_tracker::DataTracker;
//!
//! #[derive(Clone,PartialEq,Debug)]
//! struct StoreType {
//!     val: i32,
//! }
//!
//! fn main() {
//!
//!     // Create our DataTracker instance.
//!     let data_store = DataTracker::new(StoreType { val: 123 });
//!     // Wrap it so we can clone it.
//!     let data_store_rc = Rc::new(RefCell::new(data_store));
//!     // Create a listener futures::Stream to receive all changes.
//!     let rx = data_store_rc.borrow_mut().add_listener();
//!     // For each change notification, do this.
//!     let rx_printer = rx.for_each(|(old_value, new_value)| {
//!                                      // In this example, we just verify things work.
//!                                      assert!(old_value.val == 123);
//!                                      assert!(new_value.val == 124);
//!                                      futures::future::err(()) // return error to abort stream
//!                                  });
//!
//!     // Create an instance of a tokio reactor.
//!     let mut core = Core::new().unwrap();
//!
//!     // Clone our DataTracker instance.
//!     let dsclone = data_store_rc.clone();
//!     // Create a timeout and then, when it fires, update the data store.
//!     let cause_change = Timeout::new(Duration::from_millis(0), &core.handle())
//!         .into_future()
//!         .flatten()
//!         .and_then(move |_| {
//!             {
//!                 let mut data_store = dsclone.borrow_mut();
//!                 let mut scoped_store = data_store.as_tracked_mut();
//!                 assert!((*scoped_store).val == 123);
//!                 (*scoped_store).val += 1;
//!             }
//!             Ok(())
//!         })
//!         .map_err(|_| ());
//!
//!     // Run our futures in the tokio event loop.
//!     core.handle().spawn(cause_change);
//!     match core.run(rx_printer) {
//!         Ok(_) => unreachable!(),
//!         Err(()) => (),
//!     }
//!
//!     // Check that the value was incremented.
//!     assert!(data_store_rc.borrow().as_ref().val == 124);
//! }
//! ```

extern crate futures;

use futures::sync::mpsc;
use futures::{Future, Sink};

/// Allow viewing and modifying data owned by `DataTracker`.
///
/// Implemented as an RAII structure that will notify listeners on drop.
///
/// Create an instance of this by calling
/// [`DataTracker::as_tracked_mut()`](./struct.DataTracker.html#method.as_tracked_mut).
pub struct Modifier<'a, T>
    where T: 'a + Clone + PartialEq
{
    orig_copy: T,
    inner_ref: &'a mut Inner<T>,
}

impl<'a, T> Modifier<'a, T>
    where T: 'a + Clone + PartialEq
{
    fn new(inner: &'a mut Inner<T>) -> Modifier<'a, T> {
        let orig_copy: T = inner.value.clone();
        Modifier {
            orig_copy: orig_copy,
            inner_ref: inner,
        }
    }
}

impl<'a, T> std::ops::Deref for Modifier<'a, T>
    where T: 'a + Clone + PartialEq
{
    type Target = T;

    fn deref(&self) -> &T {
        &self.inner_ref.value
    }
}

impl<'a, T> std::ops::DerefMut for Modifier<'a, T>
    where T: 'a + Clone + PartialEq
{
    fn deref_mut(&mut self) -> &mut T {
        &mut self.inner_ref.value
    }
}

impl<'a, T> Drop for Modifier<'a, T>
    where T: 'a + Clone + PartialEq
{
    fn drop(&mut self) {
        if self.orig_copy != self.inner_ref.value {
            let orig_value = self.orig_copy.clone(); // TODO use std::mem::replace?
            let new_value = self.inner_ref.value.clone();
            self.inner_ref.notify_listeners(orig_value, new_value);
        }
    }
}

// holds the actual store in `value`
struct Inner<T>
    where T: Clone + PartialEq
{
    value: T,
    tx_map: Vec<mpsc::Sender<(T, T)>>,
}

impl<T> Inner<T>
    where T: Clone + PartialEq
{
    fn notify_listeners(&mut self, orig_value: T, new_value: T) {
        let mut to_return = Vec::new();
        let orig_map = std::mem::replace(&mut self.tx_map, Vec::new());
        for on_changed_tx in orig_map.into_iter() {
            match on_changed_tx
                      .send((orig_value.clone(), new_value.clone()))
                      .wait() { // TODO remove .wait() here
                Ok(t) => to_return.push(t),
                Err(_) => continue,
            }
        }
        for el in to_return.into_iter() {
            self.tx_map.push(el);
        }
    }
}

/// Tracks changes to data and notifies listeners.
///
/// The data to be tracked is type `T`.
///
/// Subsribe to changes by calling `add_listener`.
///
/// See the [module-level documentation](./) for more details.
pub struct DataTracker<T>
    where T: Clone + PartialEq
{
    inner: Inner<T>,
}

impl<T> DataTracker<T>
    where T: Clone + PartialEq
{
    /// Create a new `DataTracker` which takes ownership
    /// of the data of type `T`.
    pub fn new(value: T) -> DataTracker<T> {
        DataTracker {
            inner: Inner {
                value: value,
                tx_map: Vec::new(),
            },
        }
    }

    /// Add a callback that will be called just after a data change is detected.
    ///
    /// Returns a Receiver which will receive messages whenever a change occurs.
    ///
    /// To remove a listener, drop the Receiver.
    pub fn add_listener(&mut self) -> mpsc::Receiver<(T, T)> {
        let (tx, rx) = mpsc::channel(1);
        self.inner.tx_map.push(tx);
        rx
    }

    /// Return a `Modifier` which can be used to modify the owned data.
    pub fn as_tracked_mut(&mut self) -> Modifier<T> {
        Modifier::new(&mut self.inner)
    }
}

impl<T> AsRef<T> for DataTracker<T>
    where T: Clone + PartialEq
{
    fn as_ref(&self) -> &T {
        &self.inner.value
    }
}
