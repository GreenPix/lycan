use std::fmt::{self,Formatter};
use std::cell::{RefCell,RefMut,Ref};
use std::marker;

use nalgebra::{Pnt2,Vec2};
use rand;

use id::{Id,HasForgeableId};
use data::{Map,Player};
use messages::{EntityState};

mod status;
mod update;
//mod serialize;

pub use self::update::update;
pub use lycan_serialize::Order;

use lycan_serialize::Direction;

static DEFAULT_SPEED: f32 = 10.0;

#[derive(Debug)]
pub struct Entity {
    id: Id<Entity>,
    data: EntityData,
}

// Everything that we will save on disk
#[derive(Debug)]
struct EntityData {
    player: EntityType,
    position: Pnt2<f32>,
    // We probably won't save the speed ...
    speed: Vec2<f32>,
    orientation: Direction,
    skin: u64,
    pv: u64,
    // Hitbox
    stats: Stats,

    // TODO: Replace by a FSM
    walk: bool,
    attacking: u64, // XXX: This is currently expressed in tick, not ms!
}

impl EntityData {
    pub fn new(player: EntityType,
               position: Pnt2<f32>,
               orientation: Direction,
               stats: Stats,
               pv: u64,
               )
        -> EntityData {
            EntityData {
                player: player,
                position: position,
                speed: Vec2::new(0.0,0.0),
                orientation: orientation,
                stats: stats,
                skin: rand::random(),
                pv: pv,

                walk: false,
                attacking: 0,
            }
        }

    pub fn dump(&self, f: &mut Formatter, indent: &str) -> Result<(),fmt::Error> {
        match self.player {
            EntityType::Player(ref player) => {
                try!(writeln!(f, "{}Player {} {} attached to map {}",
                              indent,
                              player.get_id(),
                              player.get_name(),
                              player.get_map_position()));
            }
            EntityType::Invoked(ref parent) => {
                match *parent {
                    Some(parent) => {
                        try!(writeln!(f, "{}Invoked entity attached to {}", indent, parent));
                    }
                    None => {
                        try!(writeln!(f, "{}Invoked entity", indent));
                    }
                }
            }
        }
        // TODO: Presence ...
        try!(writeln!(f, "{}{:?} {:?} {:?}", indent, self.position, self.speed,self.orientation));
        writeln!(f, "{}PV: {}", indent, self.pv)
    }
}

impl Entity {
    pub fn get_id(&self) -> Id<Self> {
        self.id
    }

    pub fn new(player: EntityType,
               position: Pnt2<f32>,
               orientation: Direction,
               stats: Stats,
               pv: u64,
               ) -> Entity {
                   Entity::new_internal(
                       EntityData::new(player, position, orientation, stats, pv))
    }

    fn new_internal(data: EntityData) -> Entity {
        Entity {
            id: Id::new(),
            data: data,
        }
    }

    pub fn get_position(&self) -> Pnt2<f32> {
        self.data.position
    }

    pub fn get_skin(&self) -> u64 {
        self.data.skin
    }

    pub fn get_pv(&self) -> u64 {
        self.data.pv
    }

    // TODO: Remove
    pub fn fake_player(id: Id<Player>) -> Entity {
        let player =  match id.as_u64() {
            0 => {
                Player::new(id, Id::forge(1), "Vaelden".to_string())
            }
            1 => {
                Player::new(id, Id::forge(1), "Cendrais".to_string())
            }
            2 => {
                Player::new(id, Id::forge(1), "Nemikolh".to_string())
            }
            _ => {
                let name = format!("Player{}", id);
                Player::new(id, Id::forge(1), name)
            }
        };
        Entity::new(EntityType::Player(player),
                    Pnt2::new(0.0,0.0),
                    Direction::North,
                    Stats{speed: DEFAULT_SPEED},
                    100)
    }

    pub fn dump(&self, f: &mut Formatter, indent: &str) -> Result<(),fmt::Error> {
        try!(writeln!(f, "{}Entity {}", indent, self.id));
        self.data.dump(f, indent)
    }

    pub fn to_entity_state(&self) -> EntityState {
        EntityState::new(self.id, self.data.position, self.data.orientation)
    }

    pub fn get_map_position(&self) -> Option<Id<Map>> {
        match self.data.player {
            EntityType::Player(ref player) => Some(player.get_map_position()),
            _ => None,
        }
    }
}

#[derive(Debug)]
pub enum EntityType {
    // An entity can be a player
    Player(Player),
    // Or invoked, with an optional parent
    // XXX: Is the parent really useful?
    Invoked(Option<u64>),
}

#[derive(Debug,Default)]
pub struct Stats {
    speed: f32,
}

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

    pub fn get_mut_wrapper<'a>(&'a mut self, id: Id<Entity>) -> Option<(&'a mut Entity, Wrapper<'a>)> {
        self.get_position(id).map(move |position| {
            Wrapper::new(&mut self.entities, position).unwrap()
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

    pub fn iter_mut_wrapper(&mut self) -> IterMutWrapper {
        IterMutWrapper {
            inner: &mut self.entities,
            current_position: 0,
        }
    }
}

pub struct Wrapper<'a> {
    inner: &'a mut [Entity],
    borrowed_entity_position: usize,
}

impl <'a> Wrapper<'a> {
    pub fn new(a: &'a mut [Entity], position: usize) -> Option<(&'a mut Entity, Wrapper<'a>)> {
        let entity: &mut Entity = unsafe {
            match a.get_mut(position) {
                None => return None,
                Some(entity) => ::std::mem::transmute(entity),
            }
        };
        let wrapper = Wrapper {
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

    pub fn iter(&mut self) -> WrapperIter {
        let p = self.inner.as_mut_ptr();
        unsafe {
            WrapperIter {
                ptr: p,
                end: p.offset(self.inner.len() as isize) ,
                borrowed_entity: p.offset(self.borrowed_entity_position as isize),
                _marker: marker::PhantomData,
            }
        }
    }

    // XXX: We should probably have a &self version
    pub fn get_position(&mut self, id: Id<Entity>) -> Option<usize> {
        for (position, entity) in self.iter().enumerate() {
            if entity.get_id() == id {
                return Some(position);
            }
        }
        None
    }
}

// TODO: Have a *const version
pub struct WrapperIter<'a> {
    ptr: *mut Entity,
    end: *mut Entity,
    borrowed_entity: *mut Entity,
    _marker: marker::PhantomData<&'a mut Entity>,
}

impl <'a> Iterator for WrapperIter<'a> {
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

pub struct IterMutWrapper<'a> {
    inner: &'a mut [Entity],
    current_position: usize,
}

// Cannot implement Iterator because an item borrows the iterator
impl <'a> IterMutWrapper<'a> {
    pub fn next_item<'b>(&'b mut self) -> Option<(&'b mut Entity, Wrapper<'b>)> {
        let res = Wrapper::new(self.inner, self.current_position);
        self.current_position += 1;
        res
    }
}

#[cfg(test)]
mod test {
    use super::{Entity, EntityStore};
    use id::Id;
    #[test]
    fn test() {
        let mut store = EntityStore::new();
        store.push(Entity::fake_player(Id::forge(0)));
        store.push(Entity::fake_player(Id::forge(1)));
        store.push(Entity::fake_player(Id::forge(2)));
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
