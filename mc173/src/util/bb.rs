//! Cube bounding boxes.

use std::ops::{Add, AddAssign, Sub, SubAssign};
use std::fmt;

use glam::DVec3;

use super::Face;


/// An axis-aligned bounding box.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct BoundingBox {
    pub min: DVec3,
    pub max: DVec3,
}

impl BoundingBox {

    pub const CUBE: Self = Self { min: DVec3::ZERO, max: DVec3::ONE };

    /// Construct a new bounding box from the minimum and maximum points.
    pub const fn new(min_x: f64, min_y: f64, min_z: f64, max_x: f64, max_y: f64, max_z: f64) -> Self {
        Self {
            min: DVec3::new(min_x, min_y, min_z),
            max: DVec3::new(max_x, max_y, max_z),
        }
    }

    /// Calculate the size of this bounding box.
    pub fn size(self) -> DVec3 {
        self.max - self.min
    }

    /// Calculate the center of the bounding box.
    pub fn center(self) -> DVec3 {
        (self.min + self.max) / 2.0
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

    /// Return true if this bounding box intersects with the given one on the X axis.
    pub fn intersects_x(self, other: Self) -> bool {
        other.max.x > self.min.x && other.min.x < self.max.x
    }

    /// Return true if this bounding box intersects with the given one on the Y axis.
    pub fn intersects_y(self, other: Self) -> bool {
        other.max.y > self.min.y && other.min.y < self.max.y
    }

    /// Return true if this bounding box intersects with the given one on the Z axis.
    pub fn intersects_z(self, other: Self) -> bool {
        other.max.z > self.min.z && other.min.z < self.max.z
    }

    /// Return true if this bounding box contains the given point.
    pub fn contains(self, point: DVec3) -> bool {
        point.x > self.min.x && point.x < self.max.x &&
        point.y > self.min.y && point.y < self.max.y &&
        point.z > self.min.z && point.z < self.max.z
    }

    /// Return true if the point is contained in this bounding box on Y/Z axis only.
    pub fn contains_yz(self, point: DVec3) -> bool {
        point.y >= self.min.y && point.y <= self.max.y && 
        point.z >= self.min.z && point.z <= self.max.z
    }

    /// Return true if the point is contained in this bounding box on X/Z axis only.
    pub fn contains_xz(self, point: DVec3) -> bool {
        point.x >= self.min.x && point.x <= self.max.x && 
        point.z >= self.min.z && point.z <= self.max.z
    }

    /// Return true if the point is contained in this bounding box on X/Y axis only.
    pub fn contains_xy(self, point: DVec3) -> bool {
        point.x >= self.min.x && point.x <= self.max.x && 
        point.y >= self.min.y && point.y <= self.max.y
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

    /// Compute an intersection of a ray into this bounding box. If this ray intersects
    /// this box, the new vector that hit the first face is returned.
    pub fn calc_ray_trace(self, origin: DVec3, ray: DVec3) -> Option<(DVec3, Face)> {

        if ray.x * ray.x >= 1e-7 {

            let (factor, face) =
            if ray.x > 0.0 { // We can collide only with NegX face.
                ((self.min.x - origin.x) / ray.x, Face::NegX)
            } else { // We can collide only with PosX face.
                ((self.max.x - origin.x) / ray.x, Face::PosX)
            };

            let point = origin + ray * factor;
            if self.contains_yz(point) {
                return Some((point - origin, face))
            }

        }

        if ray.y * ray.y >= 1e-7 {

            let (factor, face) =
            if ray.y > 0.0 { // We can collide only with NegY face.
                ((self.min.y - origin.y) / ray.y, Face::NegY)
            } else { // We can collide only with PosY face.
                ((self.max.y - origin.y) / ray.y, Face::PosY)
            };

            let point = origin + ray * factor;
            if self.contains_xz(point) {
                return Some((point - origin, face))
            }

        }

        if ray.z * ray.z >= 1e-7 {

            let (factor, face) =
            if ray.z > 0.0 { // We can collide only with NegZ face.
                ((self.min.z - origin.z) / ray.z, Face::NegZ)
            } else { // We can collide only with PosZ face.
                ((self.max.z - origin.z) / ray.z, Face::PosZ)
            };

            let point = origin + ray * factor;
            if self.contains_xy(point) {
                return Some((point - origin, face))
            }

        }

        None

    }

}

impl Add<DVec3> for BoundingBox {
    type Output = BoundingBox;
    #[inline]
    fn add(self, rhs: DVec3) -> Self::Output {
        self.offset(rhs)
    }
}

impl AddAssign<DVec3> for BoundingBox {
    #[inline]
    fn add_assign(&mut self, rhs: DVec3) {
        *self = self.offset(rhs);
    }
}

impl Sub<DVec3> for BoundingBox {
    type Output = BoundingBox;
    #[inline]
    fn sub(self, rhs: DVec3) -> Self::Output {
        self.offset(-rhs)
    }
}

impl SubAssign<DVec3> for BoundingBox {
    #[inline]
    fn sub_assign(&mut self, rhs: DVec3) {
        *self = self.offset(-rhs);
    }
}

impl fmt::Display for BoundingBox {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "[{}, {}]", self.min, self.max)
    }
}
