use nalgebra::Point2;
use quad_tree::Rectangle;

#[derive(Debug,Clone,Copy)]
pub struct RectangleHitbox {
    half_width: f32,
    half_height: f32,
}

impl RectangleHitbox {
    pub fn new(h_width: f32, h_height: f32) -> RectangleHitbox {
        RectangleHitbox {
            half_width: h_width,
            half_height: h_height,
        }
    }

    // TODO: Need to check units, and put a sensible default
    pub fn new_default() -> RectangleHitbox {
        RectangleHitbox::new(0.75,1.0)
    }

    pub fn collision(&self, pos: Point2<f32>, other: &RectangleHitbox, pos_other: Point2<f32>) -> bool {
        self.to_rectangle(pos).intersects(&other.to_rectangle(pos_other))
    }

    pub fn rotated(&self) -> RectangleHitbox {
        RectangleHitbox {
            half_width: self.half_height,
            half_height: self.half_width,
        }
    }

    pub fn to_rectangle(&self, pos: Point2<f32>) -> Rectangle {
        Rectangle::new(pos.x - self.half_width,
                       pos.y - self.half_height,
                       pos.x + self.half_width,
                       pos.y + self.half_height)
    }
}
