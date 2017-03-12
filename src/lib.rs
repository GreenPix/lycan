#![feature(conservative_impl_trait)]
#![feature(mpsc_select)]
#![feature(fnbox)]

#![allow(unused_imports)]
#![allow(dead_code)]

extern crate rustc_serialize;
#[macro_use] extern crate lazy_static;
#[macro_use] extern crate log;
//extern crate id;
extern crate time;
//extern crate mio;
extern crate byteorder;
extern crate threadpool;
extern crate lycan_serialize;
extern crate nalgebra;
extern crate smallvec;
extern crate rand;
extern crate aariba;
extern crate bytes;
extern crate behaviour_tree;
extern crate serde;
extern crate serde_json;
#[macro_use] extern crate serde_derive;
extern crate uuid;
extern crate futures;
extern crate futures_cpupool;
extern crate tokio_core;
extern crate schedule_recv;
#[macro_use] extern crate error_chain;

// Iron and related crates
#[macro_use] extern crate iron;
#[macro_use] extern crate hyper;
extern crate router;
extern crate bodyparser;
extern crate plugin;
extern crate modifier;
//extern crate iron_error_router;
extern crate mount;

pub mod actor;
pub mod entity;
pub mod game;
pub mod instance;
pub mod id;
pub mod data;
pub mod ai;
pub mod utils;
mod scripts;
mod network;

pub mod messages;
