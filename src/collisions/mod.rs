use quad_tree::QuadTree;
use quad_tree::Rectangle;
use nalgebra::Point2;
use nalgebra::Vector2;

use id::Id;
use data::Map as DataMap;
use instance::SEC_PER_UPDATE;

use self::hitbox::RectangleHitbox;

pub mod hitbox;

// TODO: Find a better place?
pub struct Map {
    pub id: Id<DataMap>,
    pub obstacles: Obstacles,
}

impl Map {
    pub fn new(data_map: Id<DataMap>) -> Map {
        // TODO: Derive obtacles from map data
        let area = Rectangle::new(-100.0, -100.0, 100.0, 100.0);
        let obstacles = vec![
            Obstacle::new(-10, 0),
            Obstacle::new(10, 10),
            Obstacle::new(10, 14),
            Obstacle::new(10, 18),
            Obstacle::new(10, 22),
            Obstacle::new(10, 26),
        ];
        Map {
            id: data_map,
            obstacles: Obstacles::new(obstacles, area),
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
    fn new(x: i16, y: i16) -> Obstacle {
        Obstacle {
            position: Point2::new(x as f32, y as f32),
            hitbox: RectangleHitbox::new(0.5, 0.5),
        }
    }
}

