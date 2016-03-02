#![feature(fnbox)]
#![feature(ip_addr)]

#![allow(unused_imports)]
#![allow(dead_code)]

extern crate rustc_serialize;
#[macro_use] extern crate lazy_static;
#[macro_use] extern crate log;
//extern crate id;
extern crate time;
extern crate mio;
extern crate byteorder;
extern crate threadpool;
extern crate lycan_serialize;
extern crate nalgebra;
extern crate smallvec;
extern crate rand;
extern crate aariba;
extern crate hyper;
extern crate bytes;
extern crate behaviour_tree;

pub mod actor;
pub mod entity;
pub mod game;
pub mod instance;
pub mod id;
pub mod data;
pub mod ai;
mod scripts;
mod network;

pub mod messages;
