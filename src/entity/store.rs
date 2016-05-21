use id::{Id, WeakId};
use super::Entity;
use super::{OthersAccessor,DoubleIterMut};

// Abstraction so that if we change the implementation it doesn't affect the rest
#[derive(Debug)]
pub struct EntityStore {
    entities: Vec<Entity>,
}

impl EntityStore {
    pub fn new() -> EntityStore {
        EntityStore {
            entities: Vec::new(),
        }
    }

    pub fn push(&mut self, entity: Entity) {
        self.entities.push(entity)
    }

    pub fn remove<T: Into<WeakId<Entity>>>(&mut self, id: T) -> Option<Entity> {
        let position = match self.get_position(id.into()) {
            Some(pos) => pos,
            None => return None,
        };

        Some(self.entities.remove(position))
    }

    pub fn remove_if<T,F>(&mut self, id: T, f: F) -> Option<Entity>
    where T: Into<WeakId<Entity>>,
          F: FnOnce(&Entity) -> bool {
        let position = match self.get_position(id.into()) {
            Some(pos) => pos,
            None => return None,
        };

        if f(self.entities.get(position).unwrap()) {
            Some(self.entities.remove(position))
        } else {
            None
        }
    }

    pub fn get<T: Into<WeakId<Entity>>>(&self, id: T) -> Option<&Entity> {
        self.get_position(id.into()).map(move |position| self.entities.get(position).unwrap())
    }

    pub fn get_mut<T: Into<WeakId<Entity>>>(&mut self, id: T) -> Option<&mut Entity> {
        self.get_position(id.into()).map(move |position| self.entities.get_mut(position).unwrap())
    }

    pub fn get_mut_wrapper<'a,T: Into<WeakId<Entity>>>(&'a mut self, id: T) -> Option<(&'a mut Entity, OthersAccessor<'a>)> {
        self.get_position(id.into()).map(move |position| {
            OthersAccessor::new(&mut self.entities, position).unwrap()
        })
    }

    fn get_position(&self, id: WeakId<Entity>) -> Option<usize> {
        for (position, entity) in self.entities.iter().enumerate() {
            if WeakId::from(entity.get_id()) == id {
                return Some(position);
            }
        }
        None
    }

    pub fn iter(&self) -> ::std::slice::Iter<Entity> {
        self.entities.iter()
    }

    pub fn iter_mut(&mut self) -> ::std::slice::IterMut<Entity> {
        self.entities.iter_mut()
    }

    pub fn iter_mut_wrapper(&mut self) -> DoubleIterMut {
        DoubleIterMut::new(&mut self.entities)
    }
}

