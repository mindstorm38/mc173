//! Tree generation functions.

use glam::IVec3;

use crate::block;
use crate::world::World;


/// Kind of tree.
pub enum TreeKind {
    Oak,
    Birch,
    Taiga,
}


/// Grow a tree at given position in a world, returning true if successful. If the tree
/// is an oak, there is a probability of 1/10 that it will be a big tree.
pub fn grow_tree_at(world: &mut World, pos: IVec3, kind: TreeKind, from_sapling: bool) -> bool {
    // TODO: Big tree.
    match kind {
        TreeKind::Oak => grow_simple_tree(world, pos, 4, from_sapling, 0),
        TreeKind::Birch => grow_simple_tree(world, pos, 5, from_sapling, 2),
        TreeKind::Taiga => grow_spruce_tree(world, pos, from_sapling),
    }
}

/// Check if a tree can grow based on some common properties.
fn check_tree(
    world: &mut World, 
    pos: IVec3, 
    height: i32,
    from_sapling: bool,
    check_radius: impl Fn(i32) -> i32,
) -> bool {

    let max_y = pos.y + height + 1;
    if pos.y < 1 || max_y >= 128 {
        return false;
    }

    // NOTE: This also ensure that our chunk is loaded.
    if !matches!(world.get_block(pos - IVec3::Y), Some((block::GRASS | block::DIRT, _))) {
        return false;
    }

    // Just check if there is enough space for the tree to grow.
    // NOTE: Skip the dy == 0 block because its a sapling.
    for y in pos.y..=max_y {

        // If we are growing from a sapling, just ignore the bottom.
        if y == pos.y && from_sapling {
            continue;
        }

        let check_radius = check_radius(y);
        for x in pos.x - check_radius..=pos.x + check_radius {
            for z in pos.z - check_radius..=pos.z + check_radius {
                if let Some((block::AIR | block::LEAVES, _)) = world.get_block(IVec3::new(x, y, z)) {
                    continue;
                }
                return false;
            }
        }

    }

    true

}

fn grow_simple_tree(world: &mut World, pos: IVec3, min_height: i32, from_sapling: bool, metadata: u8) -> bool {

    let height = world.get_rand_mut().next_int_bounded(3) + min_height;

    let check_radius = |y| {
        if y == pos.y {
            0
        } else if y >= pos.y + height - 1 {
            2
        } else {
            1
        }
    };

    if !check_tree(world, pos, height, from_sapling, check_radius) {
        return false;
    }

    world.set_block(pos - IVec3::Y, block::DIRT, 0);

    for y in (pos.y + height - 3)..=(pos.y + height) {

        let dy = y - (pos.y + height);  // Delta from top of the tree.
        let radius = 1 - dy / 2;

        for x in pos.x - radius..=pos.x + radius {
            for z in pos.z - radius..=pos.z + radius {
                let dx = (x - pos.x).abs();
                let dz = (z - pos.z).abs();
                if dx != radius || dz != radius || (world.get_rand_mut().next_int_bounded(2) != 0 && dy != 0) {
                    let replace_pos = IVec3::new(x, y, z);
                    if !world.is_block_opaque_cube(replace_pos) {
                        world.set_block(replace_pos, block::LEAVES, metadata);
                    }
                }
            }
        }

    }

    for y in pos.y..(pos.y + height) {
        let replace_pos = IVec3::new(pos.x, y, pos.z);
        if let Some((block::AIR | block::LEAVES | block::SAPLING, _)) = world.get_block(replace_pos) {
            world.set_block(replace_pos, block::LOG, metadata);
        }
    }

    true

}

fn grow_spruce_tree(world: &mut World, pos: IVec3, from_sapling: bool) -> bool {

    let height = world.get_rand_mut().next_int_bounded(4) + 6;
    let leaves_offset = world.get_rand_mut().next_int_bounded(2) + 1;
    let leaves_height = height - leaves_offset;
    let max_radius = world.get_rand_mut().next_int_bounded(2) + 2;

    let leaves_y = pos.y + leaves_offset;

    let check_radius = |y| {
        if y < leaves_y {
            0
        } else {
            max_radius
        }
    };

    if !check_tree(world, pos, height, from_sapling, check_radius) {
        return false;
    }

    world.set_block(pos - IVec3::Y, block::DIRT, 0);

    let mut current_radius = world.get_rand_mut().next_int_bounded(2);
    let mut start_radius = 0;
    let mut global_radius = 1;

    for dy in 0..=leaves_height {

        let y = pos.y + height - dy;

        for x in pos.x - current_radius..=pos.x + current_radius {
            for z in pos.z - current_radius..=pos.z + current_radius {
                let dx = (x - pos.x).abs();
                let dz = (z - pos.z).abs();
                if dx != current_radius || dz != current_radius || current_radius <= 0 {
                    let replace_pos = IVec3::new(x, y, z);
                    if !world.is_block_opaque_cube(replace_pos) {
                        world.set_block(replace_pos, block::LEAVES, 1);
                    }
                }
            }
        }

        if current_radius >= global_radius {
            current_radius = start_radius;
            start_radius = 1;
            global_radius = max_radius.min(global_radius + 1);
        } else {
            current_radius += 1;
        }

    }

    let log_offset = world.get_rand_mut().next_int_bounded(3);
    for y in pos.y..(pos.y + height - log_offset) {
        let replace_pos = IVec3::new(pos.x, y, pos.z);
        if let Some((block::AIR | block::LEAVES | block::SAPLING, _)) = world.get_block(replace_pos) {
            world.set_block(replace_pos, block::LOG, 1);
        }
    }

    true

}
