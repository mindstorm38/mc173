//! Various shortcut methods to directly check block materials.

use glam::IVec3;

use crate::block;

use super::World;


impl World {

    /// Return true if the block at given position can be replaced.
    pub fn is_block_replaceable(&mut self, pos: IVec3) -> bool {
        if let Some((id, _)) = self.get_block(pos) {
            block::from_id(id).material.is_replaceable()
        } else {
            false
        }
    }

    /// Return true if the block at position is opaque.
    pub fn is_block_opaque(&mut self, pos: IVec3) -> bool {
        if let Some((id, _)) = self.get_block(pos) {
            block::material::is_opaque_cube(id)
        } else {
            false
        }
    }

    /// Return true if the block at position is material solid.
    pub fn is_block_solid(&mut self, pos: IVec3) -> bool {
        if let Some((id, _)) = self.get_block(pos) {
            block::from_id(id).material.is_solid()
        } else {
            false
        }
    }

    /// Return true if the block at position is air.
    #[inline]
    pub fn is_block_air(&mut self, pos: IVec3) -> bool {
        matches!(self.get_block(pos), Some((block::AIR, _)))
    }

    /// Return true if the block at position is the given one. 
    #[inline]
    pub fn is_block(&mut self, pos: IVec3, id: u8) -> bool {
        if let Some((pos_id, _)) = self.get_block(pos) {
            pos_id == id
        } else {
            false
        }
    }

}
