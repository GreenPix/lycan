use std::marker;

use entity::Entity;
use id::Id;

pub struct OthersAccessor<'a> {
    inner: &'a mut [Entity],
    borrowed_entity_position: usize,
}

impl <'a> OthersAccessor<'a> {
    pub fn new(a: &'a mut [Entity], position: usize) -> Option<(&'a mut Entity, OthersAccessor<'a>)> {
        let entity: &mut Entity = unsafe {
            match a.get_mut(position) {
                None => return None,
                Some(entity) => ::std::mem::transmute(entity),
            }
        };
        let wrapper = OthersAccessor {
            inner: a,
            borrowed_entity_position: position
        };
        Some((entity, wrapper))
    }

    pub fn get_by_index(&mut self, index: usize) -> Option<&mut Entity> {
        if index == self.borrowed_entity_position {
            None
        } else {
            self.inner.get_mut(index)
        }
        /*
        let entity = self.inner.get(
        let a: &mut [T] = unsafe { mem::transmute(self.inner as *mut [T]) };
        a.get_mut(index)
        */
    }

    pub fn get(&mut self, id: Id<Entity>) -> Option<&mut Entity> {
        match self.get_position(id) {
            Some(pos) => self.get_by_index(pos),
            None => None,
        }
    }

    pub fn iter_mut(&mut self) -> OthersIterMut {
        let p = self.inner.as_mut_ptr();
        unsafe {
            OthersIterMut {
                ptr: p,
                end: p.offset(self.inner.len() as isize) ,
                borrowed_entity: p.offset(self.borrowed_entity_position as isize),
                _marker: marker::PhantomData,
            }
        }
    }

    pub fn iter(&self) -> OthersIter {
        let p = self.inner.as_ptr();
        unsafe {
            OthersIter {
                ptr: p,
                end: p.offset(self.inner.len() as isize) ,
                borrowed_entity: p.offset(self.borrowed_entity_position as isize),
                _marker: marker::PhantomData,
            }
        }
    }

    // XXX: We should probably have a &self version
    pub fn get_position(&self, id: Id<Entity>) -> Option<usize> {
        let borrowed = self.borrowed_entity_position;
        for (position, entity) in self.iter().enumerate() {
            if entity.get_id() == id {
                let adjusted_position = if position >= borrowed {
                    position + 1
                } else {
                    position
                };
                return Some(adjusted_position);
            }
        }
        None
    }
}

// TODO: Have a *const version
pub struct OthersIterMut<'a> {
    ptr: *mut Entity,
    end: *mut Entity,
    borrowed_entity: *mut Entity,
    _marker: marker::PhantomData<&'a mut Entity>,
}

impl <'a> Iterator for OthersIterMut<'a> {
    type Item = &'a mut Entity;

    fn next(&mut self) -> Option<<Self as Iterator>::Item> {
        if self.ptr == self.end {
            None
        } else {
            let old = self.ptr;
            self.ptr = unsafe { self.ptr.offset(1) };
            if old == self.borrowed_entity {
                self.next()
            } else {
                unsafe { Some(::std::mem::transmute(old)) }
            }
        }
    }
}

pub struct OthersIter<'a> {
    ptr: *const Entity,
    end: *const Entity,
    borrowed_entity: *const Entity,
    _marker: marker::PhantomData<&'a Entity>,
}

impl <'a> Iterator for OthersIter<'a> {
    type Item = &'a Entity;

    fn next(&mut self) -> Option<<Self as Iterator>::Item> {
        if self.ptr == self.end {
            None
        } else {
            let old = self.ptr;
            self.ptr = unsafe { self.ptr.offset(1) };
            if old == self.borrowed_entity {
                self.next()
            } else {
                unsafe { Some(::std::mem::transmute(old)) }
            }
        }
    }
}

pub struct DoubleIterMut<'a> {
    inner: &'a mut [Entity],
    current_position: usize,
}

// Cannot implement Iterator because an item borrows the iterator
impl <'a> DoubleIterMut<'a> {
    pub fn next_item<'b>(&'b mut self) -> Option<(&'b mut Entity, OthersAccessor<'b>)> {
        let res = OthersAccessor::new(self.inner, self.current_position);
        self.current_position += 1;
        res
    }

    pub fn new(e: &mut [Entity]) -> DoubleIterMut {
        DoubleIterMut {
            inner: e,
            current_position: 0,
        }
    }
}

#[cfg(test)]
mod test {
    use uuid::Uuid;
    use entity::{Entity, EntityStore};
    use data::Player;
    use id::Id;
    #[test]
    fn test() {
        let mut store = EntityStore::new();
        store.push(Entity::from(Player::default_player(Id::forge(Uuid::nil()))));
        store.push(Entity::from(Player::default_player(Id::forge(Uuid::nil()))));
        store.push(Entity::from(Player::default_player(Id::forge(Uuid::nil()))));
        {
            let mut double_iter = store.iter_mut_wrapper();
            while let Some((entity,mut wrapper)) = double_iter.next_item() {
                let id = entity.get_id();
                for other in wrapper.iter() {
                    assert!(id != other.get_id());
                }
                assert!(wrapper.get(id).is_none());
            }
        }
    }
}

