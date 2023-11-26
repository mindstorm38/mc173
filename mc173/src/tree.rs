//! Tree generation functions.

use glam::IVec3;

use crate::block;
use crate::util::JavaRandom;
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
        TreeKind::Oak => {
            if world.get_rand_mut().next_int_bounded(10) == 0 {
                grow_big_tree(world, pos, from_sapling, &BigTreeConfig::default())
            } else {
                grow_simple_tree(world, pos, 4, from_sapling, 0)
            }
        }
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

pub fn grow_simple_tree(world: &mut World, pos: IVec3, min_height: i32, from_sapling: bool, metadata: u8) -> bool {

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

pub fn grow_spruce_tree(world: &mut World, pos: IVec3, from_sapling: bool) -> bool {

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



pub struct BigTreeConfig {
    height_range: i32,
    height_attenuation: f32,
    leaf_density: f32,
    branch_delta_height: i32,
    branch_scale: f32,
    branch_slope: f32,
}

impl Default for BigTreeConfig {
    fn default() -> Self {
        Self { 
            height_range: 12, 
            height_attenuation: 0.618, 
            leaf_density: 1.0,
            branch_delta_height: 4,
            branch_scale: 1.0,
            branch_slope: 0.381,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct BigTreeNode {
    /// Center of the leaves node.
    pos: IVec3,
    /// Start Y position on the main branch.
    start_y: i32,
}


pub fn grow_big_tree(world: &mut World, pos: IVec3, from_sapling: bool, config: &BigTreeConfig) -> bool {

    let mut rand = JavaRandom::new(world.get_rand_mut().next_long());
    let mut height = rand.next_int_bounded(config.height_range) + 5;

    if !matches!(world.get_block(pos - IVec3::Y), Some((block::GRASS | block::DIRT, _))) {
        return false;
    }

    // Check that we can grow the main branch.
    let main_branch_from = if from_sapling { pos + IVec3::Y } else { pos };
    let main_branch_to = pos + IVec3::new(0, height, 0);
    match check_big_tree_branch(world, main_branch_from, main_branch_to) {
        Some(new_to) => {
            // If the new length is too short, abort.
            if new_to.y - pos.y < 6 {
                return false;
            } else {
                height = new_to.y - pos.y;
            }
        }
        None => {}
    }

    // Now we grow the main branch and generate all branches.
    let mut height_attenuated = (height as f32 * config.height_attenuation) as i32;
    if height_attenuated >= height {
        height_attenuated = height - 1;
    }

    // Calculate the maximum nodes count.
    let nodes_per_height = ((1.382 + (config.leaf_density * height as f32 / 13.0).powi(2)) as i32).max(1) as usize;

    // IDEA: Use a thread local vector.
    let mut nodes = Vec::with_capacity(nodes_per_height * height as usize);

    // Current leaf offset, this will be decreased.
    let mut leaf_offset = height - config.branch_delta_height;
    let mut leaf_y = pos.y + leaf_offset;
    let start_y = pos.y + height_attenuated;

    // First node is mandatory and is the center one.
    nodes.push(BigTreeNode {
        pos: IVec3::new(pos.x, leaf_y, pos.z),
        start_y,
    });

    leaf_y -= 1;

    while leaf_offset >= 0 {

        let size = calc_big_tree_layer_size(leaf_offset, height);
        if size >= 0.0 {

            for _ in 0..nodes_per_height {

                let length = config.branch_scale * size as f32 * (rand.next_float() + 0.328);
                let angle = rand.next_float() * 2.0 * 3.14159;

                let leaf_x = (length * angle.sin() + pos.x as f32 + 0.5).floor() as i32;
                let leaf_z = (length * angle.cos() + pos.z as f32 + 0.5).floor() as i32;

                let leaf_pos = IVec3::new(leaf_x, leaf_y, leaf_z);
                let leaf_check_pos = leaf_pos + IVec3::new(0, config.branch_delta_height, 0);
                if check_big_tree_branch(world, leaf_pos, leaf_check_pos).is_none() {

                    // We compute the horizontal distance to the leaf from the main 
                    // branch, the branch will start this distance subtracted to leaf Y.
                    // This cause the branch slope to be 45 degrees, we then applies a
                    // slop factor from the config that will eventually reduce or increase
                    // the slope angle.
                    let horiz_dist = (((pos.x as f32 - leaf_x as f32).powi(2) + (pos.z as f32 - leaf_z as f32).powi(2))).sqrt();
                    let leaf_start_delta = horiz_dist * config.branch_slope;

                    // Do not go below global start Y.
                    let leaf_start_y = ((leaf_y as f32 - leaf_start_delta) as i32).min(start_y);
                    
                    let leaf_start_pos = IVec3::new(pos.x, leaf_start_y, pos.z);
                    if check_big_tree_branch(world, leaf_start_pos, leaf_pos).is_none() {
                        nodes.push(BigTreeNode {
                            pos: leaf_pos,
                            start_y: leaf_start_y,
                        });
                    }
                    
                }

            }

        }

        leaf_y -= 1;
        leaf_offset -= 1;

    }

    // Place all the leaves blocks.
    for node in &nodes {
        place_big_tree_leaf(world, node.pos, config);
    }

    // Place the main branch.
    place_big_tree_branch(world, pos, pos + IVec3::new(0, height_attenuated, 0));
    
    // Place all branches.
    let min_height = height as f32 * 0.2;
    for node in &nodes {
        if (node.start_y - pos.y) as f32 >= min_height || true {
            place_big_tree_branch(world, IVec3::new(pos.x, node.start_y, pos.z), node.pos);
        }
    }

    true

}

/// Grow a big tree leaf ball of leaves.
fn place_big_tree_leaf(world: &mut World, pos: IVec3, config: &BigTreeConfig) {
    for dy in 0..config.branch_delta_height {
        let radius = if dy != 0 && dy != config.branch_delta_height - 1 { 3.0 } else { 2.0 };
        place_big_tree_leaf_layer(world, pos + IVec3::new(0, dy, 0), radius);
    }
}

/// Grow a single horizontal layer of leaves of given radius.
fn place_big_tree_leaf_layer(world: &mut World, pos: IVec3, radius: f32) {

    let block_radius = (radius + 0.618) as i32;

    for dx in -block_radius..=block_radius {
        for dz in -block_radius..=block_radius {
            let dist = ((dx.abs() as f32 + 0.5).powi(2) + (dz.abs() as f32 + 0.5).powi(2)).sqrt();
            if dist <= radius {
                let replace_pos = pos + IVec3::new(dx, 0, dz);
                if let Some((block::AIR | block::LEAVES, _)) = world.get_block(replace_pos) {
                    world.set_block(replace_pos, block::LEAVES, 0);
                }
            }
        }
    }

}

/// Place a branch from a position to another one.
fn place_big_tree_branch(world: &mut World, from: IVec3, to: IVec3) {
    for pos in BlockLineIter::new(from, to) {
        world.set_block(pos, block::LOG, 0);
    }
}

/// Check a big tree branch, this function returns the first position on the line 
/// that is not valid for growing a branch.
/// If none is returned then the branch is fully valid.
fn check_big_tree_branch(world: &mut World, from: IVec3, to: IVec3) -> Option<IVec3> {

    for pos in BlockLineIter::new(from, to) {
        if !matches!(world.get_block(pos), Some((block::AIR | block::LEAVES, _))) {
            return Some(pos);
        }
    }

    None

}

fn calc_big_tree_layer_size(leaf_offset: i32, height: i32) -> f32 {

    if (leaf_offset as f64) < (height as f64 * 0.3) {
        return -1.618;  // Seems to be a joke value because it's never used.
    }

    let a = height as f32 / 2.0;
    let b = a - leaf_offset as f32;

    (if b == 0.0 {
        a
    } else if b.abs() >= a {
        0.0
    } else {
        (a.abs().powi(2) - b.abs().powi(2)).sqrt()
    }) * 0.5

}


/// Internal iterator for iterating all blocks of a straight line between two points.
#[derive(Default)]
struct BlockLineIter {
    from: IVec3,
    major_axis: usize,
    second_axis: usize,
    third_axis: usize,
    second_ratio: f32,
    third_ratio: f32,
    major_inc: i32,
    major_max: i32,
    major: i32,
}

impl BlockLineIter {

    fn new(from: IVec3, to: IVec3) -> Self {

        let delta = to - from;
        if delta == IVec3::ZERO {
            return Self::default();
        }

        // Find the axis with the maximum delta, our operations will be based on it.
        let major_axis = (0..3).map(|i: usize| (i, delta[i].abs())).max_by_key(|&(_, delta)| delta).unwrap().0;
        let second_axis = (major_axis + 1) % 3;
        let third_axis = (major_axis + 2) % 3;

        let major_delta = delta[major_axis];
        let second_ratio = delta[second_axis] as f32 / major_delta as f32;
        let third_ratio = delta[third_axis] as f32 / major_delta as f32;
        
        let major_inc = major_delta.signum();
        let major_max = major_delta + major_inc;

        Self {
            from,
            major_axis,
            second_axis,
            third_axis,
            second_ratio,
            third_ratio,
            major_inc,
            major_max,
            major: 0,
        }
        
    }

}

impl Iterator for BlockLineIter {

    type Item = IVec3;

    fn next(&mut self) -> Option<Self::Item> {
        if self.major == self.major_max {
            None
        } else {
            let mut pos = IVec3::ZERO;
            // PARITY: The Notchian client adds an offset of 0.5 before floor only when
            // placing logs, the first issue is that it's incoherent with the check 
            // function, so here we remove the offset for now.
            pos[self.major_axis] = self.from[self.major_axis] + self.major;
            pos[self.second_axis] = (self.from[self.second_axis] as f32 + self.major as f32 * self.second_ratio).floor() as i32;
            pos[self.third_axis] = (self.from[self.third_axis] as f32 + self.major as f32 * self.third_ratio).floor() as i32;
            self.major += self.major_inc;
            Some(pos)
        }
    }

}
