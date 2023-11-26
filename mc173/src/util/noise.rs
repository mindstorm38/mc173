//! Perlin and octaves noise generators.

use glam::{DVec3, DVec2};

use super::JavaRandom;


/// A 3D/2D Perlin noise generator.
#[derive(Debug, Clone)]
pub struct PerlinNoise {
    /// All permutations used by Perlin noise algorithm.
    permutations: Box<[u16; 512]>,
    /// Offset applied to all position given to the generator.
    offset: DVec3,
}

impl PerlinNoise {

    /// Create a new perlin noise initialized with the given RNG.
    pub fn new(rand: &mut JavaRandom) -> Self {

        let offset = rand.next_dvec3() * 256.0;
        let mut permutations = Box::new(std::array::from_fn::<u16, 512, _>(|i| {
            if i <= 256 {
                i as u16
            } else {
                0
            }
        }));

        for index in 0usize..256 {
            let permutation_index = rand.next_int_bounded(256 - index as i32) as usize + index;
            permutations.swap(index, permutation_index);
            permutations[index + 256] = permutations[index];
        }

        Self {
            permutations,
            offset,
        }

    }

    /// Get the noise value at given 3D coordinates.
    pub fn gen_3d_point(&self, pos: DVec3) -> f64 {

        let mut pos = pos + self.offset;
        let pos_floor = pos.floor();
        pos -= pos_floor;
        let factor = pos * pos * pos * (pos * (pos * 6.0 - 15.0) + 10.0);
        
        let pos_int = pos_floor.as_ivec3();
        let x_index = (pos_int.x & 255) as usize;
        let y_index = (pos_int.y & 255) as usize;
        let z_index = (pos_int.z & 255) as usize;

        let a = self.permutations[x_index] as usize + y_index;
        let a0 = self.permutations[a] as usize + z_index;
        let a1 = self.permutations[a + 1] as usize + z_index;
        let b = self.permutations[x_index + 1] as usize + y_index;
        let b0 = self.permutations[b] as usize + z_index;
        let b1 = self.permutations[b + 1] as usize + z_index;

        lerp(factor.z,
            lerp(factor.y, 
                lerp(factor.x, 
                    grad(self.permutations[a0], pos), 
                    grad(self.permutations[b0], pos - DVec3::new(1.0, 0.0, 0.0))), 
                lerp(factor.x,
                    grad(self.permutations[a1], pos - DVec3::new(0.0, 1.0, 0.0)),
                    grad(self.permutations[b1], pos - DVec3::new(1.0, 1.0, 0.0)))),
            lerp(factor.y,
                lerp(factor.x,
                    grad(self.permutations[a0 + 1], pos - DVec3::new(0.0, 0.0, 1.0)),
                    grad(self.permutations[b0 + 1], pos - DVec3::new(1.0, 0.0, 1.0))),
                lerp(factor.x,
                    grad(self.permutations[a1 + 1], pos - DVec3::new(0.0, 1.0, 1.0)),
                    grad(self.permutations[b1 + 1], pos - DVec3::new(1.0, 1.0, 1.0)))))

    }

    /// Get the noise value at given 2D coordinates.
    pub fn gen_2d_point(&self, pos: DVec2) -> f64 {
        self.gen_3d_point(pos.extend(0.0))
    }

    pub fn gen_3d<const X: usize, const Y: usize, const Z: usize>(&self, 
        cube: &mut NoiceCube<X, Y, Z>,
        offset: DVec3,
        scale: DVec3,
        freq: f64
    ) {
        
        let freq_inverted = 1.0 / freq;
        let mut last_y_index = usize::MAX;

        let mut x0 = 0.0;
        let mut x1 = 0.0;
        let mut x2 = 0.0;
        let mut x3 = 0.0;

        for x_cube in 0..X {
            let (x, x_factor, x_index) = calc_pos((offset.x + x_cube as f64) * scale.x + self.offset.x);
            for z_cube in 0..Z {
                let (z, z_factor, z_index) = calc_pos((offset.z + z_cube as f64) * scale.z + self.offset.z);
                for y_cube in 0..Y {
                    let (y, y_factor, y_index) = calc_pos((offset.y + y_cube as f64) * scale.y + self.offset.y);

                    if y_cube == 0 || y_index != last_y_index {
                        
                        last_y_index = y_index;

                        let a = self.permutations[x_index] as usize + y_index;
                        let a0 = self.permutations[a] as usize + z_index;
                        let a1 = self.permutations[a + 1] as usize + z_index;
                        let b = self.permutations[x_index + 1] as usize + y_index;
                        let b0 = self.permutations[b] as usize + z_index;
                        let b1 = self.permutations[b + 1] as usize + z_index;

                        let pos = DVec3::new(x, y, z);

                        x0 = lerp(x_factor, 
                            grad(self.permutations[a0], pos), 
                            grad(self.permutations[b0], pos - DVec3::new(1.0, 0.0, 0.0)));
                        x1 = lerp(x_factor,
                            grad(self.permutations[a1], pos - DVec3::new(0.0, 1.0, 0.0)),
                            grad(self.permutations[b1], pos - DVec3::new(1.0, 1.0, 0.0)));
                        x2 = lerp(x_factor,
                            grad(self.permutations[a0 + 1], pos - DVec3::new(0.0, 0.0, 1.0)),
                            grad(self.permutations[b0 + 1], pos - DVec3::new(1.0, 0.0, 1.0)));
                        x3 = lerp(x_factor,
                            grad(self.permutations[a1 + 1], pos - DVec3::new(0.0, 1.0, 1.0)),
                            grad(self.permutations[b1 + 1], pos - DVec3::new(1.0, 1.0, 1.0)));

                    }

                    let noise = lerp(z_factor, lerp(y_factor, x0, x1), lerp(y_factor, x2, x3));
                    cube.add(x_cube, y_cube, z_cube, noise * freq_inverted);

                }
            }
        }

    }

}

#[inline]
fn lerp(factor: f64, from: f64, to: f64) -> f64 {
    from + factor * (to - from)
}

#[inline]
fn grad(value: u16, pos: DVec3) -> f64 {
    let value = value & 15;
    let a = if value < 8 { pos.x } else { pos.y };
    let b = if value < 4 { pos.y } else if value != 12 && value != 14 { pos.z } else { pos.x };
    (if value & 1 == 0 { a } else { -a }) + (if value & 2 == 0 { b } else { -b })
}

#[inline]
fn calc_pos(mut pos: f64) -> (f64, f64, usize) {

    let floor = pos.floor();
    pos -= floor;
    let factor = pos * pos * pos * (pos * (pos * 6.0 - 15.0) + 10.0);
    let index = (floor as i32 & 255) as usize;

    (pos, factor, index)

}


/// A Perlin-based octave noise generator.
#[derive(Debug, Clone)]
pub struct OctaveNoise {
    /// Collection of generators for the different octaves.
    generators: Box<[PerlinNoise]>
}

impl OctaveNoise {

    /// Create a new Perlin-based octaves noise generator.
    pub fn new(rand: &mut JavaRandom, octaves: usize) -> Self {
        Self {
            generators: (0..octaves)
                .map(move |_| PerlinNoise::new(rand))
                .collect::<Vec<_>>()
                .into_boxed_slice(),
        }
    }

    /// Get the noise value at given 3D coordinates.
    pub fn gen_3d_point(&self, pos: DVec3) -> f64 {
        let mut ret = 0.0;
        let mut freq = 1.0;
        for gen in &self.generators[..] {
            ret += gen.gen_3d_point(pos * freq) / freq;
            freq /= 2.0;
        }
        ret
    }

    /// Get the noise value at given 3D coordinates.
    pub fn gen_2d_point(&self, pos: DVec2) -> f64 {
        let mut ret = 0.0;
        let mut freq = 1.0;
        for gen in &self.generators[..] {
            ret += gen.gen_2d_point(pos * freq) / freq;
            freq /= 2.0;
        }
        ret
    }

    pub fn gen_3d<const X: usize, const Y: usize, const Z: usize>(&self, 
        cube: &mut NoiceCube<X, Y, Z>,
        offset: DVec3,
        scale: DVec3
    ) {

        cube.fill(0.0);

        let mut freq = 1.0;

        for gen in &self.generators[..] {
            gen.gen_3d(cube, offset, scale * freq, freq);
            freq /= 2.0;
        }

    }

}


/// A cube of given size for storing some noise values.
pub struct NoiceCube<const X: usize, const Y: usize, const Z: usize> {
    inner: [[[f64; Y]; Z]; X],
}

impl<const X: usize, const Y: usize, const Z: usize> NoiceCube<X, Y, Z> {

    #[inline]
    pub fn fill(&mut self, value: f64) {
        self.inner = [[[value; Y]; Z]; X];
    }

    #[inline]
    pub fn get(&mut self, x: usize, y: usize, z: usize) -> f64 {
        self.inner[x][z][y]
    }

    #[inline]
    pub fn set(&mut self, x: usize, y: usize, z: usize, value: f64) {
        self.inner[x][z][y] = value;
    }

    #[inline]
    pub fn add(&mut self, x: usize, y: usize, z: usize, value: f64) {
        self.inner[x][z][y] += value;
    }

}