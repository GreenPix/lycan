use nalgebra::Pnt2;

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

    pub fn collision(&self, pos: Pnt2<f32>, other: &RectangleHitbox, pos_other: Pnt2<f32>) -> bool {
        let left1  = pos.x       - self.half_width;
        let right1 = pos.x       + self.half_width;
        let left2  = pos_other.x - other.half_width;
        let right2 = pos_other.x + other.half_width;

        let bot1 = pos.y       - self.half_height;
        let top1 = pos.y       + self.half_height;
        let bot2 = pos_other.y - other.half_height;
        let top2 = pos_other.y + other.half_height;

        if bot1 > top2 { return false; }
        if top1 < bot2 { return false; }

        if right1 < left2  { return false; }
        if left1  > right2 { return false; }

        true
    }

    pub fn rotated(&self) -> RectangleHitbox {
        RectangleHitbox {
            half_width: self.half_height,
            half_height: self.half_width,
        }
    }
}
