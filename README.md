# raii-change-tracker - tracks changes to data and notifies listeners [![Version][version-img]][version-url] [![Status][status-img]][status-url] [![Doc][doc-img]][doc-url]

**Update: the package has been deprecated in favor of https://github.com/astraw/async-change-tracker/**

## Documentation

Documentation is available [here](https://docs.rs/raii-change-tracker/).

Track changes to data and notify listeners.

The main API feature is the `DataTracker`
struct, which takes ownership of a value.

To listen for changes to an instance of the `DataTracker` struct, the function
`DataTracker::add_listener()` returns a `futures::Stream`. This `Stream` is
supplied with previous and new values immediately after each change and
integrates with the [tokio](https://tokio.rs) ecosystem.

The principle of operation is that `DataTracker::as_tracked_mut()` returns a
mutable `Modifier`. `Modifier` is a RAII scoped guard with two key properties:

- It implements the
  [`DerefMut`](https://doc.rust-lang.org/std/ops/trait.DerefMut.html) trait to
  allow ergonomic access to the underlying data.
- It implements the [`Drop`](https://doc.rust-lang.org/std/ops/trait.Drop.html)
  trait which checks if the underlying data was changed and, if so, notifies the
  listeners.

Futhermore, `DataTracker::as_ref()` returns a (non-mutable) reference to the
data for cases when only read-only access to the data is needed.

To implement tracking, when `Modifier` is created, a copy of the original data
is made and when `Modifier` is dropped, an equality check is performed. If the
original and the new data are not equal, the callbacks are called with
references to the old and the new values.

## License

Licensed under either of

* Apache License, Version 2.0,
  (./LICENSE-APACHE or http://www.apache.org/licenses/LICENSE-2.0)
* MIT license (./LICENSE-MIT or http://opensource.org/licenses/MIT)
  at your option.

## Contribution

Unless you explicitly state otherwise, any contribution intentionally
submitted for inclusion in the work by you, as defined in the Apache-2.0
license, shall be dual licensed as above, without any additional terms or
conditions.

## Code of conduct

Anyone who interacts with raii-change-tracker in any space including but not
limited to this GitHub repository is expected to follow our [code of
conduct](https://github.com/astraw/raii-change-tracker/blob/master/code_of_conduct.md).

[version-img]: https://img.shields.io/crates/v/raii-change-tracker.svg
[version-url]: https://crates.io/crates/raii-change-tracker
[status-img]: https://travis-ci.org/astraw/raii-change-tracker.svg?branch=master
[status-url]: https://travis-ci.org/astraw/raii-change-tracker
[doc-img]: https://docs.rs/raii-change-tracker/badge.svg
[doc-url]: https://docs.rs/raii-change-tracker/
