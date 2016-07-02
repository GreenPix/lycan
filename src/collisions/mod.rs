use std::iter;

use quad_tree::QuadTree;
use quad_tree::Rectangle;
use nalgebra::Point2;
use nalgebra::Vector2;

use id::Id;
use data::Map as DataMap;
use instance::SEC_PER_UPDATE;

use self::hitbox::RectangleHitbox;
use self::pathfinding::AStarTiles;

pub mod hitbox;
mod pathfinding;

// Convention: only positive positions
// (0.0, 0.0) is the bottom left corner

// TODO: Find a better place?
pub struct Map {
    pub id: Id<DataMap>,
    pub obstacles: Obstacles,
    pub pathfinding_tiles: AStarTiles,
}

impl Map {
    pub fn new(data_map: Id<DataMap>) -> Map {
        // TODO: Derive obtacles from map data
        let width = 100;
        let height = 100;
        let obstacle_pos = [
            (10, 10),
            (10, 14),
            (10, 18),
            (10, 22),
            (10, 26),
        ];

        let mut tiles: Vec<u8> = iter::repeat(1).take(width*height).collect();
        for &(x, y) in obstacle_pos.iter() {
            tiles[y * width + x] = 0;
        }

        let area = Rectangle::new(0.0, 0.0, width as f32, height as f32);
        let obstacle_list = obstacle_pos.iter().map(|&(x,y)| Obstacle::new(x,y)).collect();

        Map {
            id: data_map,
            pathfinding_tiles: AStarTiles::new(tiles, width),
            obstacles: Obstacles::new(obstacle_list, area),
        }
    }
}

pub struct Obstacles {
    quad_tree: QuadTree<ObstaclesData>,
}

struct ObstaclesData {
    num: u16,
    hitbox: RectangleHitbox,
}

impl Obstacles {
    fn new(list: Vec<Obstacle>, area: Rectangle) -> Obstacles {
        let mut tree = QuadTree::new(area);
        for (ind, obs) in list.iter().enumerate() {
            tree.add(obs.position, ObstaclesData {
                num: ind as u16,
                hitbox: obs.hitbox
            });
        }
        Obstacles {
            quad_tree: tree,
        }
    }

    /// Resolves collisions with obstacles
    ///
    /// Returns new position and adjusted speed
    pub fn resolve_collision(&self,
                             current_pos: Point2<f32>,
                             hitbox: RectangleHitbox,
                             current_speed: Vector2<f32>,
                             ) -> (Point2<f32>, Vector2<f32>) {
        // Currently very simple: if we collision with an obstacle, we stay where we are
        let mut collision = false;
        let next_pos = current_pos + current_speed * *SEC_PER_UPDATE;
        let next_rectangle = hitbox.to_rectangle(next_pos);
        let area = self.quad_tree.area();
        if !area.contains(next_rectangle) {
            debug!("Reached border of map");
            return (current_pos, Vector2::new(0.0,0.0));
        }
        self.quad_tree.visit(&mut |area, node| {
            match node {
                Some((position, data)) => {
                    if hitbox.collision(next_pos, &data.hitbox, position) {
                        debug!("Collision with obstacle {} {:?}", data.num, position);
                        collision = true;
                    }
                    true // ignored
                }
                None => {
                    next_rectangle.intersects_loosened(area, 0.5)
                }
            }
        });
        if collision {
            (current_pos, Vector2::new(0.0,0.0))
        } else {
            (next_pos, current_speed)
        }
    }
}

pub struct Obstacle {
    position: Point2<f32>,
    hitbox: RectangleHitbox,
}

impl Obstacle {
    // The granularity is in tiles
    fn new(x: usize, y: usize) -> Obstacle {
        Obstacle {
            position: Point2::new(x as f32 + 0.5, y as f32 + 0.5),
            hitbox: RectangleHitbox::new(0.5, 0.5),
        }
    }
}

