use std::collections::HashMap;
use std::sync::Arc;
use std::sync::mpsc::{self,Sender,Receiver};
use std::fmt;
use std::hash::Hash;

use threadpool::ThreadPool;

use id::Id;
use data::{Map,Player};
use entity::Entity;

pub struct ResourceManager {
    maps: ResourceManagerInner<Map,Arc<Map>>,
    players: ResourceManagerInner<Player,Entity>,
    pool: ThreadPool,
}

struct ResourceManagerInner<T,U> {
    resources: HashMap<Id<T>, U>,
    errors: HashMap<Id<T>, Error>,
    jobs: CurrentJobs<T>,

    // XXX: Maybe a sync receiver would be better here
    tx: Sender  <(Id<T>, Result<U, Error>)>,
    rx: Receiver<(Id<T>, Result<U, Error>)>,
}

impl <T,U> ResourceManagerInner<T,U>
where U: RetreiveFromId<T>,
      U: Send + 'static,
      T: 'static {
    fn new() -> ResourceManagerInner<T,U> {
        let (tx, rx) = mpsc::channel();
        ResourceManagerInner {
            resources: HashMap::new(),
            errors: HashMap::new(),
            jobs: CurrentJobs::new(),

            tx: tx,
            rx: rx,
        }
    }

    fn load(&mut self, id: Id<T>, pool: &ThreadPool) {
        self.process_inputs();
        if self.resources.get(&id).is_none() &&
            self.errors.get(&id).is_none() &&
            !self.jobs.contains(id) {
                self.jobs.push(id);
                let tx = self.tx.clone();
                pool.execute(move || {
                    // TODO: fetch the resource on the disk
                    let fetched = U::retrieve(id);

                    tx.send((id, fetched)).unwrap();
                });
        }
    }

    fn process_inputs(&mut self) {
        while let Ok((id, data_res)) = self.rx.try_recv() {
            match data_res {
                Ok(data) => {
                    self.jobs.remove(id);
                    if let Some(_old) = self.resources.insert(id,data) {
                        warn!("Replacing resource {} in the resource manager", id);
                    }
                }
                Err(e) => {
                    self.errors.insert(id, e);
                }
            }
        }
    }

    fn retrieve(&mut self, id: Id<T>, pool: &ThreadPool) -> Result<U, Error> {
        self.process_inputs();
        // We already have it
        if let Some(data) =  self.resources.remove(&id) {
            return Ok(data);
        }

        if self.jobs.contains(id) {
            return Err(Error::Processing);
        }

        if let Some(error) = self.errors.get(&id) {
            return Err(error.clone());
        }

        // We don't have it, not processing and no errors ... we fetch it
        self.load(id, pool);
        Err(Error::Processing)
    }
}

impl <T,U> ResourceManagerInner<T,U>
where U: RetreiveFromId<T>,
      U: Send + Clone + 'static,
      T: 'static {
    fn get(&mut self, id: Id<T>, pool: &ThreadPool) -> Result<U, Error> {
        self.process_inputs();
        // We already have it
        if let Some(data) =  self.resources.get(&id) {
            return Ok(data.clone());
        }

        if self.jobs.contains(id) {
            return Err(Error::Processing);
        }

        if let Some(error) = self.errors.get(&id) {
            return Err(error.clone());
        }

        // We don't have it, not processing and no errors ... we fetch it
        self.load(id, pool);
        Err(Error::Processing)
    }
}

impl ResourceManager {
    pub fn new(threads: usize) -> ResourceManager {
        ResourceManager {
            maps: ResourceManagerInner::new(),
            players: ResourceManagerInner::new(),
            pool: ThreadPool::new(threads),
        }
    }

    pub fn load_map(&mut self, map: Id<Map>) {
        self.maps.load(map, &self.pool);
    }

    pub fn get_map(&mut self, map: Id<Map>) -> Result<Arc<Map>, Error> {
        self.maps.get(map, &self.pool)
    }

    pub fn load_player(&mut self, player: Id<Player>) {
        self.players.load(player, &self.pool);
    }

    pub fn retrieve_player(&mut self, player: Id<Player>) -> Result<Entity, Error> {
        self.players.retrieve(player, &self.pool)
    }
}

#[derive(Debug)]
enum Data {
    Map(Map),
}

#[derive(Debug)]
struct CurrentJobs<T> {
    inner: Vec<Id<T>>,
}

impl <T> CurrentJobs<T> {
    fn new() -> CurrentJobs<T> {
        CurrentJobs {
            inner: Vec::new(),
        }
    }

    fn contains(&self, id: Id<T>) -> bool {
        self.inner.contains(&id)
    }

    fn push(&mut self, id: Id<T>) -> bool {
        if !self.inner.contains(&id) {
            self.inner.push(id);
            true
        } else {
            false
        }
    }

    fn remove(&mut self, id: Id<T>) {
        match self.inner.iter().position(|a| *a == id) {
            None => error!("Trying to remove a non-existing id {}", id),
            Some(pos) => {
                self.inner.swap_remove(pos);
            }
        }
    }
}

impl fmt::Debug for ResourceManager {
    fn fmt(&self, fmt: &mut fmt::Formatter) -> fmt::Result {
        fmt.debug_struct("ResourceManager")
            .field("maps", &self.maps)
            .field("players", &self.players)
            .finish()
    }
}

impl <T: fmt::Debug, U: fmt::Debug> fmt::Debug for ResourceManagerInner<T,U> {
    fn fmt(&self, fmt: &mut fmt::Formatter) -> fmt::Result {
        fmt.debug_struct("ResourceManagerInner")
            .field("resources", &self.resources)
            .field("errors", &self.errors)
            .field("jobs", &self.jobs)
            .finish()
    }
}

// Fetch the resource from the disk
pub trait RetreiveFromId<T=Self> {
    fn retrieve(id: Id<T>) -> Result<Self,Error> where Self: Sized;
}

#[derive(Debug,Clone,Copy)]
pub enum Error {
    Processing,
    NotFound,
}

impl RetreiveFromId<Player> for Entity {
    fn retrieve(id: Id<Player>) -> Result<Entity,Error> {
        Ok(Entity::fake_player(id))
    }
}

impl RetreiveFromId for Map {
    fn retrieve(id: Id<Map>) -> Result<Map,Error> {
        Ok(Map::new(id))
    }
}

impl <T,U> RetreiveFromId<U> for Arc<T>
where T: RetreiveFromId<U> {
    fn retrieve(id: Id<U>) -> Result<Arc<T>,Error> {
        T::retrieve(id).map(Arc::new)
    }
}
