use serde::{Deserialize,Deserializer};
use serde::de::Error;

use id::{Id, HasForgeableId, HasId};
use uuid::Uuid;

#[derive(Clone, Debug, Serialize)]
pub struct Map {
    pub uuid: Id<Map>,
    pub name: String,
    pub row_size: u16,
    pub number_row: u16,
    /// For each tile, say if the tile is passing (true) or blocking (false)
    pub tiles: Vec<bool>,
}

impl Deserialize for Map {
    fn deserialize<D: Deserializer>(d: D) -> Result<Self, D::Error> {
        let inner = MapInner::deserialize(d)?;
        if inner.tiles.len() != ((inner.row_size * inner.number_row) as usize) {
            let expected = format!("an array of size number_row * row_size = {}",
                                   inner.number_row * inner.row_size);
            return Err(D::Error::invalid_length(inner.tiles.len(), &expected.as_str()));
        }
        Ok(Map::from(inner))
    }
}

impl From<MapInner> for Map {
    fn from(inner: MapInner) -> Map {
        assert!(inner.tiles.len() == ((inner.row_size * inner.number_row) as usize));
        Map {
            uuid: inner.uuid,
            name: inner.name,
            row_size: inner.row_size,
            number_row: inner.number_row,
            tiles: inner.tiles,
        }
    }
}

// The map as represented in the backend
#[derive(Deserialize)]
struct MapInner {
    pub uuid: Id<Map>,
    pub name: String,
    pub row_size: u16,
    pub number_row: u16,
    pub tiles: Vec<bool>,
}

impl HasId for Map {
    type Type = Uuid;
}

impl HasForgeableId for Map {}

impl Map {
    pub fn get_id(&self) -> Id<Map> {
        self.uuid
    }

    pub fn get_name(&self) -> &str {
        &self.name
    }

    /// Used when the "default_fallback" flag is set. It will return a default map, with the given
    /// uuid
    pub fn default_map(uuid: Id<Map>) -> Map {
        let tiles = vec![
            true, true, true, true, true, true, true, true, true, true,
            true, true, true, true, true, true, true, true, true, true,
            true, true, true, true, true, true, true, true, true, true,
            true, true, true, true, true, true, true, true, true, true,
            true, true, true, true, true, true, true, true, true, true,
            true, true, true, true, true, true, true, true, true, true,
            true, true, true, true, true, true, true, true, true, true,
            true, true, true, true, true, true, true, true, true, true,
            true, true, true, true, true, true, true, true, true, true,
            true, true, true, true, true, true, true, true, true, true,
        ];
        Map {
            uuid: uuid,
            name: format!("Default map - {}", uuid),
            row_size: 10,
            number_row: 10,
            tiles: tiles,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::Map;
    #[test]
    fn ok() {
        let map_json = r#"
            {
                "uuid": "42424242-4242-4242-4242-424242424242",
                "name": "Default map",
                "row_size": 3,
                "number_row": 3,
                "tiles": [true, false, true, true, true, false, true, true, false]
            }
        "#;
        ::serde_json::from_str::<Map>(map_json).unwrap();
    }
    #[test]
    fn incorrect_length() {
        let map_json = r#"
            {
                "uuid": "42424242-4242-4242-4242-424242424242",
                "name": "Default map",
                "row_size": 3,
                "number_row": 3,
                "tiles": [true, false, true, true, true, false, true, true]
            }
        "#;
        assert!(::serde_json::from_str::<Map>(map_json).is_err());
    }
}
