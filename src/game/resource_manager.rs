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

use utils;
use id::{Id,HasId};
use data::{Map,Player};
use data::UNIQUE_MAP;
use entity::Entity;
use game::Game;
use messages::Request;

mod errors {
    error_chain! { }
}

pub use self::errors::*;

pub struct ResourceManager {
    inner: Box<Backend>,
}

impl ResourceManager {
    pub fn new_rest(base_url: String) -> ResourceManager {
        let backend = Box::new(RestBackend::new(base_url));
        ResourceManager {
            inner: backend,
        }
    }

    pub fn get_map(&mut self, map: Id<Map>) -> BoxFuture<Map, Error> {
        let res = if map != UNIQUE_MAP.get_id() {
            Err(format!("Map id requested different than current UNIQUE_MAP: {}", map).into())
        } else {
            Ok(UNIQUE_MAP.clone())
        };
        res.into_future().boxed()
    }

    pub fn get_player(&mut self, player: Id<Player>) -> BoxFuture<Entity, Error> {
        self.inner.get_player(player)
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
        unimplemented!();
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

