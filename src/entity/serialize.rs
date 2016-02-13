use std::fmt::Write;
use std::str;

use rustc_serialize::json::{self,ToJson};
use rustc_serialize::{Encodable,Decodable};

use entity::Entity;

use super::EntityType;

// Serialization of entities on the disk / database

pub fn serialize(_entity: Entity) -> Vec<u8> {
    /*
    match entity.data.player {
        EntityType::Invoked(Some(id)) =>
            panic!("The serialisation of children entities is not supported yet, id {}", id),
        _ => {}
    }

    let mut res = String::new();
    let json_encoder = json::as_pretty_json(&entity.data);
    write!(res, "{}", json_encoder).unwrap();
    res.into_bytes()
    */
    unimplemented!();
}

    /*
pub fn deserialize(_data: &[u8]) -> Result<Entity,String> {
    let result = str::from_utf8(data);
    match result {
        Ok(json_str) => {
            json::decode(json_str).map_err(|e| e.to_string())
                .map(|data| Entity::new_internal(data))
        }
        Err(e) => Err(e.to_string()),
    }
    unimplemented!();
}
    */
