//! TODO: This module is being contributed back to futures-rs
use std::mem;

use futures::{Future, IntoFuture, Async, Poll};
use futures::stream::Stream;

/// Converts a closure generating a future into a Stream
///
/// This adapter will continuously use a provided generator to generate a future, then wait
/// for its completion and output its result.
///
/// The generator is given a state in input, and must output a structure implementing
/// `IntoFuture`. The future must resolve to a tuple containing a new state, and a value.
/// The value will be returned by the stream, and the new state will be given to the generator
/// to create a new future.
///
/// The initial state is provided to this method
///
/// # Example
///
/// ```rust
/// use futures::*;
/// use futures::stream::Stream;
///
/// let mut stream = stream::repeat(0, |state| {
///     if state <= 2 {
///         let fut: Result<_,()> = Ok((state+1, state*2));
///         Some(fut)
///     } else {
///         None
///     }
/// });
/// assert_eq!(Ok(Async::Ready(Some(0))), stream.poll());
/// assert_eq!(Ok(Async::Ready(Some(2))), stream.poll());
/// assert_eq!(Ok(Async::Ready(Some(4))), stream.poll());
/// assert_eq!(Ok(Async::Ready(None)), stream.poll());
/// ```
pub fn repeat<T, F, Fut, It>(init: T, f: F) -> Repeat<T, F, Fut>
where F: FnMut(T) -> Option<Fut>,
      Fut: IntoFuture<Item = (T,It)> {
    Repeat {
        f: f,
        state: State::Ready(init),
    }
}

/// A stream which creates futures, polls them and return their result
///
/// This stream is returned by the `futures::stream::repeat` method
#[must_use = "streams do nothing unless polled"]
pub struct Repeat<T, F, Fut> where Fut: IntoFuture {
    f: F,
    state: State<T, Fut::Future>,
}

impl <T, F, Fut, It> Stream for Repeat<T, F, Fut>
where F: FnMut(T) -> Option<Fut>,
      Fut: IntoFuture<Item = (T,It)> {
    type Item = It;
    type Error = Fut::Error;

    fn poll(&mut self) -> Poll<Option<It>, Fut::Error> {
        loop {
            match mem::replace(&mut self.state, State::Empty) {
                State::Empty => panic!("cannot poll Repeat twice"),
                State::Ready(state) => {
                    match (self.f)(state) {
                        Some(fut) => { self.state = State::Processing(fut.into_future()); }
                        None => { return Ok(Async::Ready(None)); }
                    }
                }
                State::Processing(mut fut) => {
                    match try!(fut.poll()) {
                        Async:: Ready((state, item)) => {
                            self.state = State::Ready(state);
                            return Ok(Async::Ready(Some(item)));
                        }
                        Async::NotReady => {
                            self.state = State::Processing(fut);
                            return Ok(Async::NotReady);
                        }
                    }
                }
            }
        }
    }
}

enum State<T, F> where F: Future {
    /// Placeholder state when doing work
    Empty,

    /// Ready to generate new future; current internal state is the `T`
    Ready(T),

    /// Working on a future generated previously
    Processing(F),
}

