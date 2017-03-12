use std::collections::HashMap;
use std::sync::Arc;
use std::sync::mpsc::{self,Sender,Receiver};
use std::fmt::{self,Write};
use std::hash::Hash;

use futures::future::{self,Future,IntoFuture,BoxFuture};
use futures::sync::mpsc::UnboundedSender;
use futures_cpupool::CpuPool;
use threadpool::ThreadPool;
use serde_json;
use error_chain::ChainedError;

use utils;
use id::{Id,HasId};
use data::{Map,Player};
use entity::Entity;
use game::Game;
use messages::Request;

mod errors {
    error_chain! { }
}

pub use self::errors::*;

pub struct ResourceManager {
    inner: Box<Backend>,
    default_fallback: bool,
}

impl ResourceManager {
    /// Builds a resource manager that will fetch data from a REST API
    ///
    /// default_fallback: if a resource is not found, create a default one
    pub fn new_rest(base_url: String, default_fallback: bool) -> ResourceManager {
        let backend = Box::new(RestBackend::new(base_url));
        ResourceManager {
            inner: backend,
            default_fallback: default_fallback,
        }
    }

    // TODO: make sure the resource manager does not load the same resource twice
    /// Fetches a map by its ID using the current backend
    pub fn get_map(&mut self, map: Id<Map>) -> BoxFuture<Map, Error> {
        let fut = self.inner.get_map(map);
        if self.default_fallback {
            fut.or_else(move |e| {
                warn!("Error while loading map {}, falling back to default. {}", map, e.display());
                Ok(Map::default_map(map))
            }).boxed()
        } else {
            fut
        }
    }

    /// Fetches a player by its ID using the current backend
    pub fn get_player(&mut self, player: Id<Player>) -> BoxFuture<Entity, Error> {
        let fut = self.inner.get_player(player);
        if self.default_fallback {
            fut.or_else(move |e| {
                warn!("Error while loading player {}, falling back to default. {}", player, e.display());
                Ok(Entity::from(Player::default_player(player)))
            }).boxed()
        } else {
            fut
        }
    }
}

trait Backend {
    fn get_map(&mut self, map: Id<Map>) -> BoxFuture<Map, Error>;
    fn get_player(&mut self, player: Id<Player>) -> BoxFuture<Entity, Error>;
}

struct RestBackend {
    pool: CpuPool,
    base_url: String,
}

impl Backend for RestBackend {
    fn get_map(&mut self, map: Id<Map>) -> BoxFuture<Map, Error> {
        let url = format!("{}/maps/{}", self.base_url, map);
        self.pool.spawn(future::lazy(move || {
            let serialized_map = utils::get_file_from_url(&url)
                .chain_err(|| format!("Cannot GET {}", url))?;
            let map = serde_json::from_str::<Map>(&serialized_map)
                .chain_err(|| "Failed to deserialize map")?;
            Ok(map)
        })).boxed()
    }

    fn get_player(&mut self, player: Id<Player>) -> BoxFuture<Entity, Error> {
        let url = format!("{}/entities/{}", self.base_url, player);
        self.pool.spawn(future::lazy(move || {
            let serialized_entity = utils::get_file_from_url(&url)
                .chain_err(|| format!("Cannot GET {}", url))?;
            let entity = serde_json::from_str::<Player>(&serialized_entity)
                .chain_err(|| "Failed to deserialize entity")?;
            Ok(Entity::from(entity))
        })).boxed()
    }
}

impl RestBackend {
    fn new(base_url: String) -> RestBackend {
        RestBackend {
            base_url: base_url,
            pool: CpuPool::new_num_cpus(),
        }
    }
}

