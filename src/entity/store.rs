use id::Id;
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

    pub fn remove(&mut self, id: Id<Entity>) -> Option<Entity> {
        let position = match self.get_position(id) {
            Some(pos) => pos,
            None => return None,
        };

        Some(self.entities.remove(position))
    }

    pub fn get(&self, id: Id<Entity>) -> Option<&Entity> {
        self.get_position(id).map(move |position| self.entities.get(position).unwrap())
    }

    pub fn get_mut(&mut self, id: Id<Entity>) -> Option<&mut Entity> {
        self.get_position(id).map(move |position| self.entities.get_mut(position).unwrap())
    }

    pub fn get_mut_wrapper<'a>(&'a mut self, id: Id<Entity>) -> Option<(&'a mut Entity, OthersAccessor<'a>)> {
        self.get_position(id).map(move |position| {
            OthersAccessor::new(&mut self.entities, position).unwrap()
        })
    }

    fn get_position(&self, id: Id<Entity>) -> Option<usize> {
        for (position, entity) in self.entities.iter().enumerate() {
            if entity.get_id() == id {
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

