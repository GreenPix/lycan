// TODO: Try to contribute back to Futures?
use std::marker::PhantomData;

use futures::Future;
use futures::IntoFuture;
use futures::Async;
use futures::Poll;
use futures::stream::Stream;

/// Converts a closure generating a future into a `futures::stream::Stream`
///
/// This adapter will continuously use a provided generator to generate a future, then wait
/// for its completion and output its result.
///
/// The generator is given a state in input, and must output a structure implementing
/// `IntoFuture`. The future must resolve to both a new state, and a value.
/// The value will be returned by the stream, and the new state will be given to the generator
/// to create a new future.
///
/// One use-case of the state is to pass ownership of objects that cannot be cloned, or expensive
/// to clone (such as buffers or sockets)
///
/// The initial state is provided to this method
///
/// # Example
///
/// ```rust,ignore
/// // Define those structures / functions somewhere
/// struct Message { ... }
/// struct Error { ... }
/// fn parse_message<T: Read>(reader: T) -> BoxFuture<(T,Message),Error> { ... }
/// fn process_message(message: Message) { ... }
///
/// fn client_connected<T: Read>(reader: T) {
///     let adapter = new_adapter(reader, |reader| {
///         parse_message(reader)
///     });
///
///     adapter.for_each(|message| {
///         println!("Received new message");
///         process_message(message)
///     })
/// }
/// ```
pub fn new_adapter<S, G: Generator<S>>(init: S, mut gen: G) -> Adapter<S,G> {
    let future = gen.create(init).map(|f| f.into_future());
    Adapter {
        gen: gen,
        current_future: future,
        _phantom: PhantomData,
    }
}

/// Structure created by the `new_adapter()` method
pub struct Adapter<S, G: Generator<S>> {
    gen: G,
    current_future: Option<<<G as Generator<S>>::Output as IntoFuture>::Future>,
    _phantom: PhantomData<S>,
}

impl <S, G: Generator<S>> Stream for Adapter<S,G> {
    type Item = <<G as Generator<S>>::Item as Split>::Right;
    type Error = <<G as Generator<S>>::Output as IntoFuture>::Error;

    fn poll(&mut self) -> Poll<Option<Self::Item>, Self::Error> {
        match self.current_future.take() {
            Some(mut fut) => {
                match fut.poll() {
                    Ok(Async:: Ready(t)) => {
                        let (state, item) = t.split();
                        self.current_future = self.gen.create(state).map(|f| f.into_future());
                        Ok(Async::Ready(Some(item)))
                    }
                    Ok(Async::NotReady) => {
                        self.current_future = Some(fut);
                        Ok(Async::NotReady)
                    }
                    Err(e) => Err(e),
                }
            }
            None => {
                return Ok(Async::Ready(None));
            }
        }
    }
}

pub trait Generator<S> {
    type Item: Split<Left=S>;
    type Output: IntoFuture<Item=Self::Item>;

    fn create(&mut self, init: S) -> Option<Self::Output>;
}

pub trait Split {
    type Left;
    type Right;

    fn split(self) -> (Self::Left, Self::Right);
}

impl <U,V> Split for (U,V) {
    type Left = U;
    type Right = V;

    fn split(self) -> (U, V) { self }
}

impl <T, It, S, O> Generator<S> for T
where T: FnMut(S) -> Option<O>,
      O: IntoFuture<Item=It>,
      It: Split<Left=S> {
          type Item = It;
          type Output = O;

          fn create(&mut self, init: S) -> Option<Self::Output> {
              self(init)
          }
      }

