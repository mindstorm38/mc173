use glam::IVec3;

use super::BoundingBox;


/// Represent a cube facing.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum Face {
    NegY = 0,
    PosY = 1,
    NegZ = 2,
    PosZ = 3,
    NegX = 4,
    PosX = 5,
}

impl Face {

    /// Get a face from the yaw component of a look only.
    #[inline]
    pub fn from_yaw(yaw: f32) -> Self {
        match ((yaw * 4.0 / std::f32::consts::TAU + 0.5).floor() as i32) & 3 {
            0 => Self::PosZ,
            1 => Self::NegX,
            2 => Self::NegZ,
            3 => Self::PosX,
            _ => unreachable!()
        }
    }

    /// Get a face from the yaw and pitch components of a look.
    #[inline]
    pub fn from_look(yaw: f32, pitch: f32) -> Self {
        if pitch < -std::f32::consts::FRAC_PI_4 {
            Self::PosY
        } else if pitch > std::f32::consts::FRAC_PI_4 {
            Self::NegY
        } else {
            Self::from_yaw(yaw)
        }
    }

    #[inline]
    pub fn is_y(self) -> bool {
        matches!(self, Face::NegY | Face::PosY)
    }

    #[inline]
    pub fn is_x(self) -> bool {
        matches!(self, Face::NegX | Face::PosX)
    }

    #[inline]
    pub fn is_z(self) -> bool {
        matches!(self, Face::NegZ | Face::PosZ)
    }

    /// Get the opposite face.
    #[inline]
    pub fn opposite(self) -> Self {
        match self {
            Face::NegY => Face::PosY,
            Face::PosY => Face::NegY,
            Face::NegZ => Face::PosZ,
            Face::PosZ => Face::NegZ,
            Face::NegX => Face::PosX,
            Face::PosX => Face::NegX,
        }
    }

    /// Rotate this face horizontally to right, Y faces don't change.
    #[inline]
    pub fn rotate_right(self) -> Self {
        match self {
            Face::NegZ => Face::PosX,
            Face::PosX => Face::PosZ,
            Face::PosZ => Face::NegX,
            Face::NegX => Face::NegZ,
            _ => self
        }
    }

    /// Rotate this face horizontally to left, Y faces don't change.
    #[inline]
    pub fn rotate_left(self) -> Self {
        match self {
            Face::NegZ => Face::NegX,
            Face::NegX => Face::PosZ,
            Face::PosZ => Face::PosX,
            Face::PosX => Face::NegZ,
            _ => self
        }
    }

    /// Get the delta vector for this face.
    #[inline]
    pub fn delta(self) -> IVec3 {
        match self {
            Face::NegY => IVec3::NEG_Y,
            Face::PosY => IVec3::Y,
            Face::NegZ => IVec3::NEG_Z,
            Face::PosZ => IVec3::Z,
            Face::NegX => IVec3::NEG_X,
            Face::PosX => IVec3::X,
        }
    }

    /// Extrude a face and form a bounding box. The face is extruded toward the opposite
    /// face. The given inset allows shrinking the face toward the center axis.
    #[inline]
    pub fn extrude(self, inset: f64, depth: f64) -> BoundingBox {
        let pos = inset;
        let neg = 1.0 - inset;
        match self {
            Face::NegY => BoundingBox::new(pos, 0.0, pos, neg, depth, neg),
            Face::PosY => BoundingBox::new(pos, 1.0 - depth, pos, neg, 1.0, neg),
            Face::NegZ => BoundingBox::new(pos, pos, 0.0, neg, neg, depth),
            Face::PosZ => BoundingBox::new(pos, pos, 1.0 - depth, neg, neg, 1.0),
            Face::NegX => BoundingBox::new(0.0, pos, pos, depth, neg, neg),
            Face::PosX => BoundingBox::new(1.0 - depth, pos, pos, 1.0, neg, neg),
        }
    }

}
