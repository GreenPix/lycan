// TODO: Try to contribute back to Futures?
use std::marker::PhantomData;

use futures::Future;
use futures::IntoFuture;
use futures::Async;
use futures::Poll;
use futures::stream::Stream;

pub fn new_adapter<S, G: Generator<S>>(mut gen: G, init: S) -> Adapter<S,G> {
    let future = gen.create(init).map(|f| f.into_future());
    Adapter {
        gen: gen,
        current_future: future,
        _phantom: PhantomData,
    }
}

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

