//! Cube bounding boxes.

use std::ops::AddAssign;
use std::fmt;

use glam::DVec3;


/// An axis-aligned bounding box.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct BoundingBox {
    pub min: DVec3,
    pub max: DVec3,
}

impl BoundingBox {

    pub const CUBE: Self = Self::new(DVec3::ZERO, DVec3::ONE);

    pub const fn new(min: DVec3, max: DVec3) -> Self {
        Self {
            min,
            max,
        }
    }

    pub fn size(self) -> DVec3 {
        self.max - self.min
    }

    /// Expand this bounding box in all direction by the given delta.
    pub fn inflate(self, delta: DVec3) -> Self {
        Self {
            min: self.min - delta,
            max: self.max + delta,
        }
    }

    /// Offset this bounding box' coordinates by the given delta.
    pub fn offset(self, delta: DVec3) -> Self {
        Self {
            min: self.min + delta,
            max: self.max + delta,
        }
    }

    /// Expand this bounding box by the given delta, only in the delta's direction.
    pub fn expand(mut self, delta: DVec3) -> Self {

        if delta.x < 0.0 {
            self.min.x += delta.x;
        } else if delta.x > 0.0 {
            self.max.x += delta.x;
        }

        if delta.y < 0.0 {
            self.min.y += delta.y;
        } else if delta.y > 0.0 {
            self.max.y += delta.y;
        }

        if delta.z < 0.0 {
            self.min.z += delta.z;
        } else if delta.z > 0.0 {
            self.max.z += delta.z;
        }

        self

    }

    /// Return true if this bounding box intersects with the given one.
    pub fn intersects(self, other: Self) -> bool {
        other.max.x > self.min.x && other.min.x < self.max.x &&
        other.max.y > self.min.y && other.min.y < self.max.y &&
        other.max.z > self.min.z && other.min.z < self.max.z
    }

    /// Return true if this bounding box contains the given point.
    pub fn contains(self, point: DVec3) -> bool {
        point.x > self.min.x && point.x < self.max.x &&
        point.y > self.min.y && point.y < self.max.y &&
        point.z > self.min.z && point.z < self.max.z
    }

    /// Simulate an offset of the given bounding box by the given delta, but with this 
    /// bounding box potentially colliding with it in the way, this function will return 
    /// the new delta that avoid this collision.
    pub fn calc_x_delta(self, other: Self, mut dx: f64) -> f64 {
        if other.max.y > self.min.y && other.min.y < self.max.y {
            if other.max.z > self.min.z && other.min.z < self.max.z {
                if dx > 0.0 && other.max.x <= self.min.x {
                    dx = dx.min(self.min.x - other.max.x);
                } else if dx < 0.0 && other.min.x >= self.max.x {
                    dx = dx.max(self.max.x - other.min.x);
                }
            }
        }
        dx
    }

    /// Simulate an offset of the given bounding box by the given delta, but with this 
    /// bounding box potentially colliding with it in the way, this function will return 
    /// the new delta that avoid this collision.
    pub fn calc_y_delta(self, other: Self, mut dy: f64) -> f64 {
        if other.max.x > self.min.x && other.min.x < self.max.x {
            if other.max.z > self.min.z && other.min.z < self.max.z {
                if dy > 0.0 && other.max.y <= self.min.y {
                    dy = dy.min(self.min.y - other.max.y);
                } else if dy < 0.0 && other.min.y >= self.max.y {
                    dy = dy.max(self.max.y - other.min.y);
                }
            }
        }
        dy
    }

    /// Simulate an offset of the given bounding box by the given delta, but with this 
    /// bounding box potentially colliding with it in the way, this function will return 
    /// the new delta that avoid this collision.
    pub fn calc_z_delta(self, other: Self, mut dz: f64) -> f64 {
        if other.max.x > self.min.x && other.min.x < self.max.x {
            if other.max.y > self.min.y && other.min.y < self.max.y {
                if dz > 0.0 && other.max.z <= self.min.z {
                    dz = dz.min(self.min.z - other.max.z);
                } else if dz < 0.0 && other.min.z >= self.max.z {
                    dz = dz.max(self.max.z - other.min.z);
                }
            }
        }
        dz
    }

}

impl AddAssign<DVec3> for BoundingBox {
    #[inline]
    fn add_assign(&mut self, rhs: DVec3) {
        *self = self.offset(rhs);
    }
}

impl AddAssign<f64> for BoundingBox {
    #[inline]
    fn add_assign(&mut self, rhs: f64) {
        *self = self.offset(DVec3::splat(rhs));
    }
}

impl fmt::Display for BoundingBox {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "[{}, {}]", self.min, self.max)
    }
}
