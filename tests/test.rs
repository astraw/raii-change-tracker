extern crate futures;
extern crate tokio_core;
extern crate raii_change_tracker;

use futures::{Future, IntoFuture, Stream};
use tokio_core::reactor::{Core, Timeout};
use std::time::Duration;
use std::rc::Rc;
use std::cell::RefCell;

use raii_change_tracker::DataTracker;

#[test]
fn test_increment() {

    #[derive(Clone,PartialEq,Debug)]
    struct StoreType {
        val: i32,
    }

    let data_store_rc = Rc::new(RefCell::new(DataTracker::new(StoreType { val: 123 })));
    let rx = data_store_rc.borrow_mut().add_listener();
    let rx_printer = rx.for_each(|(old_value, new_value)| {
                                     assert!(old_value.val == 123);
                                     assert!(new_value.val == 124);
                                     futures::future::err(()) // return error to abort stream
                                 });

    let mut core = Core::new().unwrap();

    let dsclone = data_store_rc.clone();
    let cause_change = Timeout::new(Duration::from_millis(0), &core.handle())
        .into_future()
        .flatten()
        .and_then(move |_| {
            {
                let mut data_store = dsclone.borrow_mut();
                let mut scoped_store = data_store.as_tracked_mut();
                assert!((*scoped_store).val == 123);
                (*scoped_store).val += 1;
            }
            Ok(())
        })
        .map_err(|_| ());

    core.handle().spawn(cause_change);
    match core.run(rx_printer) {
        Ok(_) => unreachable!(),
        Err(()) => (),
    }

    assert!(data_store_rc.borrow().as_ref().val == 124);
}
