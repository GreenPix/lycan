use std::sync::atomic::{AtomicUsize, Ordering};
use std::fmt::{Debug,Formatter,Display,Error};
use std::hash::{Hash,Hasher};
use std::marker::{PhantomData};
use std::ops::Deref;
use std::borrow::Borrow;
use std::collections::HashSet;

use rustc_serialize::{Encodable,Encoder,Decodable,Decoder};
use serde::de::{self,Deserialize,Deserializer,Visitor};
use serde::ser::{Serialize,Serializer};

pub trait HasId {
    type Type: Hash + Eq + Send + Sync + Clone + Copy + Debug;
}

/// A Typed-ID.
pub struct Id<T: HasId> {
    inner: WeakId<T>,
}

/// A Typed-ID coming from unsure input
pub struct WeakId<T: HasId>{
    id: T::Type,
    marker: PhantomData<fn() -> T>,
}

impl<T: HasId<Type=u64>> Id<T> {
    /// Create a new Typed-ID.
    ///
    /// By design, `Id::new()` always generate an unique ID that has never been
    /// seen before.
    pub fn new() -> Id<T> {
        Id{inner: WeakId::new(NEXT_ID.fetch_add(1, Ordering::Relaxed) as u64)}
    }

    pub fn as_u64(self) -> u64 {
        self.inner.id
    }
}

impl<T: HasId> Id<T> {
    fn new_inner(weak: WeakId<T>) -> Id<T> {
        Id {
            inner: weak,
        }
    }

    pub fn into_inner(self) -> T::Type {
        self.inner.into_inner()
    }
}

impl<T: HasId> WeakId<T> {
    pub fn new(id: T::Type) -> WeakId<T> {
        WeakId{id: id, marker: PhantomData}
    }

    pub fn into_inner(self) -> T::Type {
        self.id
    }
}

impl<T: HasId<Type=u64>> WeakId<T> {
    pub fn as_u64(self) -> u64 {
        self.id
    }
}

/*
impl<T: HasId> From<T::Type> for WeakId<T> {
    fn from(id: T::Type) -> WeakId<T> {
        WeakId::new(id)
    }
}
*/

impl<T: HasId> From<Id<T>> for WeakId<T> {
    fn from(id: Id<T>) -> WeakId<T> {
        id.inner
    }
}

impl<T: HasId> Borrow<WeakId<T>> for Id<T> {
    fn borrow(&self) -> &WeakId<T> {
        &self.inner
    }
}

// TODO: Remove that!
impl<T: HasId<Type=u64>> Borrow<u64> for Id<T> {
    fn borrow(&self) -> &u64 {
        &self.inner.id
    }
}

impl<T: HasId> Clone for WeakId<T>
where T::Type: Clone {
    fn clone(&self) -> WeakId<T> {
        WeakId {
            id: self.id.clone(),
            marker: PhantomData,
        }
    }
}

impl<T: HasId> Clone for Id<T>
where T::Type: Clone {
    fn clone(&self) -> Id<T> {
        Id {
            inner: self.inner.clone(),
        }
    }
}

impl<T: HasId> Copy for WeakId<T>
where T::Type: Copy {}
impl<T: HasId> Eq for WeakId<T>
where T::Type: Eq {}
impl<T: HasId> PartialEq for WeakId<T>
where T::Type: PartialEq {
    fn eq(&self, other: &WeakId<T>) -> bool {
        self.id.eq(&other.id)
    }
}

impl<T: HasId> Hash for WeakId<T>
where T::Type: Hash {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.id.hash(state)
    }
}

impl<T: HasId> Debug for WeakId<T>
where T::Type: Debug {
    fn fmt(&self, formatter: &mut Formatter) -> Result<(), Error> {
        <T::Type as Debug>::fmt(&self.id, formatter)
    }
}

impl<T: HasId> Display for WeakId<T>
where T::Type: Display {
    fn fmt(&self, formatter: &mut Formatter) -> Result<(), Error> {
        <T::Type as Display>::fmt(&self.id, formatter)
    }
}

impl<T: HasId> Copy for Id<T>
where T::Type: Copy {}
impl<T: HasId> Eq for Id<T>
where T::Type: Eq {}
impl<T: HasId> PartialEq for Id<T>
where T::Type: PartialEq {
    fn eq(&self, other: &Id<T>) -> bool {
        self.inner.eq(&other.inner)
    }
}

impl<T: HasId> Hash for Id<T>
where T::Type: Hash {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.inner.hash(state)
    }
}

impl<T: HasId> Debug for Id<T>
where T::Type: Debug {
    fn fmt(&self, formatter: &mut Formatter) -> Result<(), Error> {
        <WeakId<T> as Debug>::fmt(&self.inner, formatter)
    }
}

impl<T: HasId> Display for Id<T>
where T::Type: Display {
    fn fmt(&self, formatter: &mut Formatter) -> Result<(), Error> {
        <WeakId<T> as Display>::fmt(&self.inner, formatter)
    }
}

lazy_static! {
    static ref NEXT_ID: AtomicUsize = AtomicUsize::new(10);
}

/// Mark a type so that it becomes possible to forge a Typed-ID for it.
pub trait HasForgeableId: HasId {}

impl<T: HasForgeableId> Id<T> {
    /// Create a new Id with the given id value
    ///
    /// Note that it is possible to create several time the same Id. In a sense,
    /// Forged Id can be seen as weaker Id.
    pub fn forge(id: T::Type) -> Id<T> {
        Id {
            inner: WeakId::new(id),
        }
    }
}

impl<T: HasForgeableId> WeakId<T> {
    pub fn upgrade(self) -> Id<T> {
        Id::new_inner(self)
    }
}

// TODO: Change type if 'id' to WeakId<T>
pub fn get_id_if_exists<T: HasId>(set: &HashSet<Id<T>>, id: T::Type) -> Option<Id<T>> {
    let weak = WeakId::new(id);
    if set.contains(&weak) {
        Some(Id{inner: weak})
    } else {
        None
    }
}

// We can transfrom Id<Self> in Id<T>
pub trait ConvertTo<T> {}

// Cannot use the From trait, as we would get conflicting implementations
impl<T: HasId> Id<T> {
    pub fn convert<U: HasId>(self) -> Id<U>
    where T: ConvertTo<U>,
          T::Type: Into<U::Type> {
        Id::new_inner(WeakId::new(self.into_inner().into()))
    }
}

impl<T: HasId> Encodable for Id<T>
where T::Type: Encodable {
    fn encode<S: Encoder>(&self, s: &mut S) -> Result<(), S::Error> {
        self.inner.encode(s)
    }
}

impl <T: HasForgeableId> Decodable for Id<T>
where T::Type: Decodable {
    fn decode<D: Decoder>(d: &mut D) -> Result<Self, D::Error> {
        WeakId::decode(d).map(Id::new_inner)
    }
}

impl <T: HasForgeableId> Deserialize for Id<T>
where T::Type: Deserialize {
    fn deserialize<D: Deserializer>(d: &mut D) -> Result<Self, D::Error> {
        WeakId::deserialize(d).map(Id::new_inner)
    }
}

impl<T: HasId> Serialize for Id<T>
where T::Type: Serialize {
    fn serialize<D: Serializer>(&self, d: &mut D) -> Result<(), D::Error> {
        self.inner.serialize(d)
    }
}

impl<T: HasId> Encodable for WeakId<T>
where T::Type: Encodable {
    fn encode<S: Encoder>(&self, s: &mut S) -> Result<(), S::Error> {
        self.id.encode(s)
    }
}

impl<T: HasId> Decodable for WeakId<T>
where T::Type: Decodable {
    fn decode<D: Decoder>(d: &mut D) -> Result<Self, D::Error> {
        Ok(WeakId {
            id: try!(T::Type::decode(d)),
            marker: PhantomData,
        })
    }
}

impl<T: HasId> Deserialize for WeakId<T>
where T::Type: Deserialize {
    fn deserialize<D: Deserializer>(d: &mut D) -> Result<Self, D::Error> {
        Ok(WeakId {
            id: try!(T::Type::deserialize(d)),
            marker: PhantomData,
        })
    }
}

impl<T: HasId> Serialize for WeakId<T>
where T::Type: Serialize {
    fn serialize<D: Serializer>(&self, d: &mut D) -> Result<(), D::Error> {
        self.id.serialize(d)
    }
}

