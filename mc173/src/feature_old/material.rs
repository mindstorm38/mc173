//! Function material functions.

use glam::IVec3;

use crate::block::material::Material;
use crate::block;

use super::World;


/// Trait extension to the world providing shortcut methods to query/check block material.
pub trait WorldMaterial: World {

    /// Get the block material at given position, defaults to air if no chunk.
    fn get_block_material(&self, pos: IVec3) -> Material {
        self.get_block(pos).map(|(id, _)| block::material::get_material(id)).unwrap_or_default()
    }

    /// Return true if the block at given position can be replaced.
    fn is_block_replaceable(&self, pos: IVec3) -> bool {
        if let Some((id, _)) = self.get_block(pos) {
            block::material::get_material(id).is_replaceable()
        } else {
            false
        }
    }

    /// Return true if the block at position is an opaque cube.
    /// 
    /// FIXME: A lot of calls to this function should instead be for "normal_cube". This
    /// is not exactly the same properties in the Notchian implementation.
    fn is_block_opaque_cube(&self, pos: IVec3) -> bool {
        if let Some((id, _)) = self.get_block(pos) {
            block::material::is_opaque_cube(id)
        } else {
            false
        }
    }

    /// Return true if the block at position is a normal cube.
    fn is_block_normal_cube(&self, pos: IVec3) -> bool {
        if let Some((id, _)) = self.get_block(pos) {
            block::material::is_normal_cube(id)
        } else {
            false
        }
    }

    /// Return true if the block at position is material solid.
    fn is_block_solid(&self, pos: IVec3) -> bool {
        if let Some((id, _)) = self.get_block(pos) {
            block::material::get_material(id).is_solid()
        } else {
            false
        }
    }

    /// Return true if the block at position is air.
    #[inline]
    fn is_block_air(&self, pos: IVec3) -> bool {
        if let Some((id, _)) = self.get_block(pos) {
            id == block::AIR
        } else {
            true
        }
    }

    /// Return true if the block at position is the given one. 
    #[inline]
    fn is_block(&self, pos: IVec3, id: u8) -> bool {
        if let Some((pos_id, _)) = self.get_block(pos) {
            pos_id == id
        } else {
            false  // TODO: id == block::AIR ? because non existing position are air
        }
    }

}

/// Standard implementation.
impl<W: World> WorldMaterial for W { }
