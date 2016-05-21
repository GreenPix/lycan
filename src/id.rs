use std::sync::atomic::{AtomicUsize, Ordering};
use std::fmt::{Debug,Formatter,Display,Error};
use std::hash::{Hash,Hasher};
use std::marker::{PhantomData};
use std::ops::Deref;
use std::borrow::Borrow;
use std::collections::HashSet;

use mio::Token;
use rustc_serialize::{Encodable,Encoder,Decodable,Decoder};
use serde::de::{self,Deserialize,Deserializer,Visitor};
use serde::ser::{Serialize,Serializer};

pub type IdImpl = u64;

/// A Typed-ID.
pub struct Id<T> {
    inner: WeakId<T>,
}

/// A Typed-ID coming from unsure input
pub struct WeakId<T>{
    id: IdImpl,
    marker: PhantomData<fn() -> T>,
}

impl <T> Id<T> {
    /// Create a new Typed-ID.
    ///
    /// By design, `Id::new()` always generate an unique ID that has never been
    /// seen before.
    pub fn new() -> Id<T> {
        Id{inner: WeakId::new(NEXT_ID.fetch_add(1, Ordering::Relaxed) as u64)}
    }

    pub fn as_token(self) -> Token {
        Token(self.inner.id as usize)
    }

    pub fn as_u64(self) -> u64 {
        self.inner.id
    }

    fn new_inner(weak: WeakId<T>) -> Id<T> {
        Id {
            inner: weak,
        }
    }
}

impl<T> WeakId<T> {
    pub fn new(id: u64) -> WeakId<T> {
        WeakId{id: id, marker: PhantomData}
    }

    pub fn as_u64(self) -> u64 {
        self.id
    }
}

impl<T> From<u64> for WeakId<T> {
    fn from(id: u64) -> WeakId<T> {
        WeakId::new(id)
    }
}

impl<T> From<Id<T>> for WeakId<T> {
    fn from(id: Id<T>) -> WeakId<T> {
        id.inner
    }
}

impl <T> Borrow<WeakId<T>> for Id<T> {
    fn borrow(&self) -> &WeakId<T> {
        &self.inner
    }
}

// TODO: Remove that!
impl <T> Borrow<u64> for Id<T> {
    fn borrow(&self) -> &u64 {
        &self.inner.id
    }
}

impl<T> Clone for WeakId<T> {
    fn clone(&self) -> WeakId<T> {
        WeakId {
            id: self.id,
            marker: PhantomData,
        }
    }
}

impl<T> Clone for Id<T> {
    fn clone(&self) -> Id<T> {
        Id {
            inner: self.inner,
        }
    }
}

impl<T> Copy for WeakId<T> {}
impl<T> Eq for WeakId<T> {}
impl<T> PartialEq for WeakId<T> {
    fn eq(&self, other: &WeakId<T>) -> bool {
        self.id.eq(&other.id)
    }
}

impl<T> Hash for WeakId<T> {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.id.hash(state)
    }
}

impl<T> Debug for WeakId<T> {
    fn fmt(&self, formatter: &mut Formatter) -> Result<(), Error> {
        <u64 as Debug>::fmt(&self.id, formatter)
    }
}

impl <T> Display for WeakId<T> {
    fn fmt(&self, formatter: &mut Formatter) -> Result<(), Error> {
        <u64 as Display>::fmt(&self.id, formatter)
    }
}

impl<T> Copy for Id<T> {}
impl<T> Eq for Id<T> {}
impl<T> PartialEq for Id<T> {
    fn eq(&self, other: &Id<T>) -> bool {
        self.inner.eq(&other.inner)
    }
}

impl<T> Hash for Id<T> {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.inner.hash(state)
    }
}

impl<T> Debug for Id<T> {
    fn fmt(&self, formatter: &mut Formatter) -> Result<(), Error> {
        <WeakId<T> as Debug>::fmt(&self.inner, formatter)
    }
}

impl <T> Display for Id<T> {
    fn fmt(&self, formatter: &mut Formatter) -> Result<(), Error> {
        <WeakId<T> as Display>::fmt(&self.inner, formatter)
    }
}

lazy_static! {
    static ref NEXT_ID: AtomicUsize = AtomicUsize::new(10);
}

/// Mark a type so that it becomes possible to forge a Typed-ID for it.
pub trait HasForgeableId {}

impl<T: HasForgeableId> Id<T> {
    /// Create a new Id with the given id value
    ///
    /// Note that it is possible to create several time the same Id. In a sense,
    /// Forged Id can be seen as weaker Id.
    pub fn forge(id: u64) -> Id<T> {
        Id {
            inner: WeakId::new(id),
        }
    }
}

// TODO: Change type if 'id' to WeakId<T>
pub fn get_id_if_exists<T>(set: &HashSet<Id<T>>, id: u64) -> Option<Id<T>> {
    if set.contains(&id) {
        Some(Id{inner: WeakId::new(id)})
    } else {
        None
    }
}

// We can transfrom Id<Self> in Id<T>
pub trait ConvertTo<T> {}

// Cannot use the From trait, as we would get conflicting implementations
impl <T> Id<T> {
    pub fn convert<U>(self) -> Id<U>
    where T: ConvertTo<U> {
        Id::new_inner(WeakId::new(self.as_u64()))
    }
}

impl <T> Encodable for Id<T> {
    fn encode<S: Encoder>(&self, s: &mut S) -> Result<(), S::Error> {
        self.inner.encode(s)
    }
}

impl <T: HasForgeableId> Decodable for Id<T> {
    fn decode<D: Decoder>(d: &mut D) -> Result<Self, D::Error> {
        WeakId::decode(d).map(Id::new_inner)
    }
}

impl <T: HasForgeableId> Deserialize for Id<T> {
    fn deserialize<D: Deserializer>(d: &mut D) -> Result<Self, D::Error> {
        WeakId::deserialize(d).map(Id::new_inner)
    }
}

impl <T> Serialize for Id<T> {
    fn serialize<D: Serializer>(&self, d: &mut D) -> Result<(), D::Error> {
        self.inner.serialize(d)
    }
}

impl <T> Encodable for WeakId<T> {
    fn encode<S: Encoder>(&self, s: &mut S) -> Result<(), S::Error> {
        s.emit_u64(self.id)
    }
}

impl <T> Decodable for WeakId<T> {
    fn decode<D: Decoder>(d: &mut D) -> Result<Self, D::Error> {
        let id = try!(d.read_u64());
        Ok(WeakId {
            id: id,
            marker: PhantomData,
        })
    }
}

impl <T> Deserialize for WeakId<T> {
    fn deserialize<D: Deserializer>(d: &mut D) -> Result<Self, D::Error> {
        let id = try!(d.deserialize_u64(U64Visitor));
        Ok(WeakId {
            id: id,
            marker: PhantomData,
        })
    }
}

impl <T> Serialize for WeakId<T> {
    fn serialize<D: Serializer>(&self, d: &mut D) -> Result<(), D::Error> {
        d.serialize_u64(self.id)
    }
}

struct U64Visitor;

impl Visitor for U64Visitor {
    type Value = u64;

    fn visit_u64<E>(&mut self, v: u64) -> Result<u64,E>
    where E: de::Error {
        Ok(v)
    }

    fn visit_i64<E>(&mut self, v: i64) -> Result<u64,E>
    where E: de::Error {
        Ok(v as u64)
    }
}
