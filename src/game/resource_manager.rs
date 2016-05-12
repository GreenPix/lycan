use std::collections::HashMap;
use std::sync::Arc;
use std::sync::mpsc::{self,Sender,Receiver};
use std::fmt::{self,Write};
use std::hash::Hash;

use threadpool::ThreadPool;
use mio::Sender as MioSender;
use serde_json;

use utils;
use id::Id;
use data::{Map,Player};
use entity::Entity;
use game::Game;
use messages::Request;

pub struct ResourceManager {
    maps: ResourceManagerInner<Map,Arc<Map>>,
    players: ResourceManagerInner<Player,Entity>,
    requests: MioSender<Request>,
    pool: ThreadPool,
    job: usize,
    base_url: String,
}

struct ResourceManagerInner<T,U> {
    resources: HashMap<Id<T>, U>,
    errors: HashMap<Id<T>, Error>,
    jobs: CurrentJobs<T>,
    requests: MioSender<Request>,

    // XXX: Maybe a sync receiver would be better here
    tx: Sender  <(Id<T>, Result<U, Error>)>,
    rx: Receiver<(Id<T>, Result<U, Error>)>,
}

impl <T,U> ResourceManagerInner<T,U>
where U: RetreiveFromId<T>,
      U: Send + 'static,
      T: 'static {
    fn new(requests: MioSender<Request>) -> ResourceManagerInner<T,U> {
        let (tx, rx) = mpsc::channel();
        ResourceManagerInner {
            resources: HashMap::new(),
            errors: HashMap::new(),
            jobs: CurrentJobs::new(),
            requests: requests,

            tx: tx,
            rx: rx,
        }
    }

    fn load(&mut self, id: Id<T>, pool: &ThreadPool, job: usize, info: U::Info) {
        self.process_inputs();
        if self.resources.get(&id).is_none() &&
            self.errors.get(&id).is_none() &&
            !self.jobs.contains(id) {
                self.jobs.push(id, job);
                let sender = self.requests.clone();
                let tx = self.tx.clone();
                pool.execute(move || {
                    // TODO: fetch the resource on the disk
                    let fetched = U::retrieve(id, info);

                    tx.send((id, fetched)).unwrap();
                    sender.send(Request::JobFinished(job)).unwrap();
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

    fn retrieve(&mut self, id: Id<T>, pool: &ThreadPool, job: usize, info: U::Info) -> Result<U, Error> {
        self.process_inputs();

        // We already have it
        if let Some(data) =  self.resources.remove(&id) {
            return Ok(data);
        }

        if let Some(job) = self.jobs.get(id) {
            return Err(Error::Processing(job));
        }

        if let Some(error) = self.errors.get(&id) {
            return Err(error.clone());
        }

        // We don't have it, not processing and no errors ... we fetch it
        self.load(id, pool, job, info);
        Err(Error::Processing(job))
    }
}

impl <T,U> ResourceManagerInner<T,U>
where U: RetreiveFromId<T>,
      U: Send + Clone + 'static,
      T: 'static {
    fn get(&mut self, id: Id<T>, pool: &ThreadPool, job: usize, info: U::Info) -> Result<U, Error> {
        self.process_inputs();
        // We already have it
        if let Some(data) =  self.resources.get(&id) {
            return Ok(data.clone());
        }

        if let Some(job) = self.jobs.get(id) {
            return Err(Error::Processing(job));
        }

        if let Some(error) = self.errors.get(&id) {
            return Err(error.clone());
        }

        // We don't have it, not processing and no errors ... we fetch it
        self.load(id, pool, job, info);
        Err(Error::Processing(job))
    }

    fn get_all(&mut self) -> Vec<U> {
        self.process_inputs();
        self.resources.values().cloned().collect()
    }
}

impl ResourceManager {
    pub fn new(threads: usize, requests: MioSender<Request>, url: String) -> ResourceManager {
        ResourceManager {
            maps: ResourceManagerInner::new(requests.clone()),
            players: ResourceManagerInner::new(requests.clone()),
            pool: ThreadPool::new(threads),
            requests: requests,
            job: 0,
            base_url: url,
        }
    }

    pub fn load_map(&mut self, map: Id<Map>) {
        let job = self.job;
        self.job += 1;
        self.maps.load(map, &self.pool, job, ());
    }

    pub fn get_map(&mut self, map: Id<Map>) -> Result<Arc<Map>, Error> {
        let job = self.job;
        self.job += 1;
        self.maps.get(map, &self.pool, job, ())
    }

    pub fn load_player(&mut self, player: Id<Player>) {
        let job = self.job;
        self.job += 1;
        self.players.load(player, &self.pool, job, self.base_url.clone());
    }

    pub fn retrieve_player(&mut self,
                           player: Id<Player>,
                          ) -> Result<Entity, Error> {
        let job = self.job;
        self.job += 1;
        self.players.retrieve(player, &self.pool, job, self.base_url.clone())
    }

    pub fn get_all_maps(&mut self) -> Vec<Arc<Map>> {
        self.maps.get_all()
    }
}

#[derive(Debug)]
enum Data {
    Map(Map),
}

#[derive(Debug)]
struct CurrentJobs<T> {
    inner: HashMap<Id<T>,usize>,
}

impl <T> CurrentJobs<T> {
    fn new() -> CurrentJobs<T> {
        CurrentJobs {
            inner: HashMap::new(),
        }
    }

    fn contains(&self, id: Id<T>) -> bool {
        self.inner.contains_key(&id)
    }

    fn get(&self, id: Id<T>) -> Option<usize> {
        self.inner.get(&id).cloned()
    }

    fn push(&mut self, id: Id<T>, job: usize) -> bool {
        if !self.inner.contains_key(&id) {
            self.inner.insert(id, job);
            true
        } else {
            false
        }
    }

    fn remove(&mut self, id: Id<T>) {
        self.inner.remove(&id);
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
    type Info: Send;
    fn retrieve(id: Id<T>, info: Self::Info) -> Result<Self,Error> where Self: Sized;
}

#[derive(Debug,Clone,Copy)]
pub enum Error {
    Processing(usize),
    NotFound,
}

impl RetreiveFromId<Player> for Entity {
    type Info = String;
    fn retrieve(id: Id<Player>, mut base: String) -> Result<Entity,Error> {
        let _ = write!(base, "/entities/{}", id);
        if let Ok(serialized_entity) = utils::get_file_from_url(&base) {
            if let Ok(entity) = serde_json::from_str::<Player>(&serialized_entity) {
                return Ok(Entity::from(entity))
            }
        }
        Ok(Entity::fake_player(id))
    }
}

impl RetreiveFromId for Map {
    type Info = ();
    fn retrieve(id: Id<Map>, _: ()) -> Result<Map,Error> {
        Ok(Map::new(id))
    }
}

impl <T,U> RetreiveFromId<U> for Arc<T>
where T: RetreiveFromId<U> {
    type Info = T::Info;
    fn retrieve(id: Id<U>, info: Self::Info) -> Result<Arc<T>,Error> {
        T::retrieve(id, info).map(Arc::new)
    }
}
