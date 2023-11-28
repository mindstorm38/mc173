//! Overworld chunk generator.

use glam::{DVec2, IVec3, Vec3Swizzles, DVec3};

use crate::util::{JavaRandom, PerlinOctaveNoise, NoiseCube};
use crate::chunk::{Chunk, CHUNK_WIDTH, CHUNK_HEIGHT};
use crate::biome::Biome;
use crate::block;

use super::cave::CaveGenerator;
use super::ChunkGenerator;


const NOISE_WIDTH: usize = 5;
const NOISE_HEIGHT: usize = 17;


/// A chunk generator for the overworld dimension.
pub struct OverworldGenerator {
    /// The random number generator used internally for chunk randomization.
    rand: JavaRandom,
    /// The noise used for generating biome temperature.
    temperature_noise: PerlinOctaveNoise,
    /// The noise used for generating biome humidity.
    humidity_noise: PerlinOctaveNoise,
    /// The noise used to alter both temperature and humidity for biome.
    biome_noise: PerlinOctaveNoise,
    terrain_noise0: PerlinOctaveNoise,
    terrain_noise1: PerlinOctaveNoise,
    terrain_noise2: PerlinOctaveNoise,
    terrain_noise3: PerlinOctaveNoise,
    terrain_noise4: PerlinOctaveNoise,
    sand_gravel_noise: PerlinOctaveNoise,
    thickness_noise: PerlinOctaveNoise,
    spawner_noise: PerlinOctaveNoise,
    cave_gen: CaveGenerator,
    biome_table: Box<[Biome; 4096]>,
    cache: Box<NoiseCache>,
}

#[derive(Default)]
struct NoiseCache {
    /// Temperature noise.
    temperature: NoiseCube<CHUNK_WIDTH, 1, CHUNK_WIDTH>,
    /// Humidity noise.
    humidity: NoiseCube<CHUNK_WIDTH, 1, CHUNK_WIDTH>,
    /// Biome noise.
    biome: NoiseCube<CHUNK_WIDTH, 1, CHUNK_WIDTH>,
    /// The final terrain noise.
    terrain:  NoiseCube<NOISE_WIDTH, NOISE_HEIGHT, NOISE_WIDTH>,
    terrain0: NoiseCube<NOISE_WIDTH, NOISE_HEIGHT, NOISE_WIDTH>,
    terrain1: NoiseCube<NOISE_WIDTH, NOISE_HEIGHT, NOISE_WIDTH>,
    terrain2: NoiseCube<NOISE_WIDTH, NOISE_HEIGHT, NOISE_WIDTH>,
    terrain3: NoiseCube<NOISE_WIDTH, 1, NOISE_WIDTH>,
    terrain4: NoiseCube<NOISE_WIDTH, 1, NOISE_WIDTH>,
    sand: NoiseCube<CHUNK_WIDTH, CHUNK_WIDTH, 1>, // Notchian server is fucking incoherent here.
    gravel: NoiseCube<CHUNK_WIDTH, 1, CHUNK_WIDTH>,
    thickness: NoiseCube<CHUNK_WIDTH, CHUNK_WIDTH, 1>,
}

impl OverworldGenerator {

    /// Create a new overworld generator given a seed.
    pub fn new(seed: i64) -> Self {

        let biome_lookup = Box::new(std::array::from_fn(|i| {
            
            let t = (i % 64) as f32 / 63.0;
            let h = (i / 64) as f32 / 63.0;
            let h = h * t;

            if t < 0.1 {
                Biome::Tundra
            } else if h < 0.2 {
                if t < 0.5 {
                    Biome::Tundra
                } else if t < 0.95 {
                    Biome::Savanna
                } else {
                    Biome::Desert
                }
            } else if h > 0.5 && t < 0.7 {
                Biome::Swampland
            } else if t < 0.5 {
                Biome::Taiga
            } else if t < 0.97 {
                if h < 0.35 {
                    Biome::ShrubLand
                } else {
                    Biome::Forest
                }
            } else if h < 0.45 {
                Biome::Plains
            } else if h < 0.9 {
                Biome::SeasonalForest
            } else {
                Biome::RainForest
            }

        }));

        let mut rand = JavaRandom::new(seed);

        Self {
            temperature_noise: PerlinOctaveNoise::new(&mut JavaRandom::new(seed.wrapping_mul(9871)), 4),
            humidity_noise: PerlinOctaveNoise::new(&mut JavaRandom::new(seed.wrapping_mul(39811)), 4),
            biome_noise: PerlinOctaveNoise::new(&mut JavaRandom::new(seed.wrapping_mul(543321)), 2),
            terrain_noise0: PerlinOctaveNoise::new(&mut rand, 16),
            terrain_noise1: PerlinOctaveNoise::new(&mut rand, 16),
            terrain_noise2: PerlinOctaveNoise::new(&mut rand, 8),
            sand_gravel_noise: PerlinOctaveNoise::new(&mut rand, 4),
            thickness_noise: PerlinOctaveNoise::new(&mut rand, 4),
            terrain_noise3: PerlinOctaveNoise::new(&mut rand, 10),
            terrain_noise4: PerlinOctaveNoise::new(&mut rand, 16),
            spawner_noise: PerlinOctaveNoise::new(&mut rand, 8),
            cave_gen: CaveGenerator::new(seed, 8),
            biome_table: biome_lookup,
            cache: Default::default(),
            rand,
        }

    }

    /// Generate a biome map for the chunk and store it in the chunk data.
    fn gen_biomes(&mut self, cx: i32, cz: i32, chunk: &mut Chunk) {

        let offset = DVec2::new((cx * 16) as f64, (cz * 16) as f64);

        let temperature = &mut self.cache.temperature;
        let humidity = &mut self.cache.humidity;
        let biome = &mut self.cache.biome;
        
        self.temperature_noise.gen_weird_2d(temperature, offset, DVec2::splat(0.025f32 as f64), 0.25);
        self.humidity_noise.gen_weird_2d(humidity, offset, DVec2::splat(0.05f32 as f64), 1.0 / 3.0);
        self.biome_noise.gen_weird_2d(biome, offset, DVec2::splat(0.25), 0.5882352941176471);

        for x in 0usize..16 {
            for z in 0usize..16 {

                let a = biome.get(x, 0, z) * 1.1 + 0.5;
                let t = (temperature.get(x, 0, z) * 0.15 + 0.7) * 0.99 + a * 0.01;
                let t = 1.0 - (1.0 - t).powi(2);
                let h = (humidity.get(x, 0, z) * 0.15 + 0.5) * 0.998 + a * 0.002;

                let t = t.clamp(0.0, 1.0);
                let h = h.clamp(0.0, 1.0);
                
                // The value may be used afterward for generation, so we update the value.
                temperature.set(x, 0, z, t);
                humidity.set(x, 0, z, h);

                let pos_biome = self.biome_table[(t * 63.0) as usize + (h * 63.0) as usize * 64];
                chunk.set_biome(IVec3::new(x as i32, 0, z as i32), pos_biome);

            }
        }

    }

    /// Generate the primitive terrain of the chunk.
    fn gen_terrain(&mut self, cx: i32, cz: i32, chunk: &mut Chunk) {

        const NOISE_STRIDE: usize = CHUNK_WIDTH / NOISE_WIDTH;
        const NOISE_REAL_WIDTH: usize = NOISE_WIDTH - 1;
        const NOISE_REAL_HEIGHT: usize = NOISE_HEIGHT - 1;
        const NOISE_REAL_WIDTH_STRIDE: usize = CHUNK_WIDTH / NOISE_REAL_WIDTH;
        const NOISE_REAL_HEIGHT_STRIDE: usize = CHUNK_HEIGHT / NOISE_REAL_HEIGHT;

        let offset = IVec3::new(cx * NOISE_REAL_WIDTH as i32, 0, cz * NOISE_REAL_WIDTH as i32);

        let terrain = &mut self.cache.terrain;
        let terrain0 = &mut self.cache.terrain0;
        let terrain1 = &mut self.cache.terrain1;
        let terrain2 = &mut self.cache.terrain2;
        let terrain3 = &mut self.cache.terrain3;
        let terrain4 = &mut self.cache.terrain4;
        let temperature = &self.cache.temperature;
        let humidity = &self.cache.humidity;

        let offset_2d = offset.xz().as_dvec2();
        let offset_3d = offset.as_dvec3();

        self.terrain_noise3.gen_2d(terrain3, offset_2d, DVec2::splat(1.121));
        self.terrain_noise4.gen_2d(terrain4, offset_2d, DVec2::splat(200.0));
        self.terrain_noise2.gen_3d(terrain2, offset_3d, DVec3::new(684.412 / 80.0, 684.412 / 160.0, 684.412 / 80.0));
        self.terrain_noise0.gen_3d(terrain0, offset_3d, DVec3::splat(684.412));
        self.terrain_noise1.gen_3d(terrain1, offset_3d, DVec3::splat(684.412));

        // Start by generating a 5x17x5 density map for the terrain.
        for x_noise in 0..NOISE_WIDTH {
            let x_block = x_noise * NOISE_STRIDE + (NOISE_STRIDE / 2);
            for z_noise in 0..NOISE_WIDTH {
                let z_block = z_noise * NOISE_STRIDE + (NOISE_STRIDE / 2);

                let t = temperature.get(x_block, 0, z_block);
                let h = humidity.get(x_block, 0, z_block) * t;
                let h_inv = (1.0 - h).powi(4);
                let h = 1.0 - h_inv;

                let mut v0 = (terrain3.get(x_noise, 0, z_noise) + 256.0) / 512.0 * h;
                v0 = v0.min(1.0);

                let mut v1 = terrain4.get(x_noise, 0, z_noise) / 8000.0;
                if v1 < 0.0 {
                    v1 = -v1 * 0.3;
                }

                v1 = v1 * 3.0 - 2.0;

                if v1 < 0.0 {
                    v1 /= 2.0;
                    v1 = v1.max(-1.0);
                    v1 /= 1.4;
                    v1 /= 2.0;
                    v0 = 0.0;
                } else {
                    v1 = v1.min(1.0);
                    v1 /= 8.0;
                }

                v0 = v0.max(0.0);
                v0 += 0.5;

                v1 = v1 * NOISE_HEIGHT as f64 / 16.0;
                let v2 = NOISE_HEIGHT as f64 / 2.0 + v1 * 4.0;

                for y_noise in 0..NOISE_HEIGHT {

                    let mut v3 = (y_noise as f64 - v2) * 12.0 / v0;
                    if v3 < 0.0 {
                        v3 *= 4.0;
                    }

                    let v4 = terrain0.get(x_noise, y_noise, z_noise) / 512.0;
                    let v5 = terrain1.get(x_noise, y_noise, z_noise) / 512.0;
                    let v6 = (terrain2.get(x_noise, y_noise, z_noise) / 10.0 + 1.0) / 2.0;

                    // NOTE: Basically a clamped linear interpolation.
                    let mut final_value = if v6 < 0.0 {
                        v4
                    } else if v6 > 1.0 {
                        v5
                    } else {
                        v4 + (v5 - v4) * v6
                    };

                    final_value -= v3;
                    if y_noise > NOISE_HEIGHT - 4 {
                        let v7 = ((y_noise - (NOISE_HEIGHT - 4)) as f32 / 3.0) as f64;
                        final_value = final_value * (1.0 - v7) + (-10.0 * v7);
                    }

                    terrain.set(x_noise, y_noise, z_noise, final_value);

                }

            }
        }

        // Then we read the generated density map and place blocks.
        for x_noise in 0..NOISE_REAL_WIDTH {
            for z_noise in 0..NOISE_REAL_WIDTH {
                for y_noise in 0..NOISE_REAL_HEIGHT {

                    let mut a = terrain.get(x_noise + 0, y_noise + 0, z_noise + 0);
                    let mut b = terrain.get(x_noise + 0, y_noise + 0, z_noise + 1);
                    let mut c = terrain.get(x_noise + 1, y_noise + 0, z_noise + 0);
                    let mut d = terrain.get(x_noise + 1, y_noise + 0, z_noise + 1);
                    let e = (terrain.get(x_noise + 0, y_noise + 1, z_noise + 0) - a) * 0.125; // Should be vectorized.
                    let f = (terrain.get(x_noise + 0, y_noise + 1, z_noise + 1) - b) * 0.125;
                    let g = (terrain.get(x_noise + 1, y_noise + 1, z_noise + 0) - c) * 0.125;
                    let h = (terrain.get(x_noise + 1, y_noise + 1, z_noise + 1) - d) * 0.125;

                    for y_index in 0..NOISE_REAL_HEIGHT_STRIDE {

                        let y = y_noise * NOISE_REAL_HEIGHT_STRIDE + y_index;
                        
                        let ca = (c - a) * 0.25;
                        let db = (d - b) * 0.25;

                        let mut a0 = a;
                        let mut b0 = b;

                        for x_index in 0..NOISE_REAL_WIDTH_STRIDE {

                            let x = x_noise * NOISE_REAL_WIDTH_STRIDE + x_index;

                            let b0a0 = (b0 - a0) * 0.25;
                            let mut a00 = a0;

                            for z_index in 0..NOISE_REAL_WIDTH_STRIDE {

                                let z = z_noise * NOISE_REAL_WIDTH_STRIDE + z_index;
                                let t = temperature.get(x, 0, z);

                                let mut id = block::AIR;

                                if y < 64 {
                                    id = if t < 0.5 && y == 63 {
                                        block::ICE
                                    } else {
                                        block::WATER_STILL
                                    };
                                }
                                
                                if a00 > 0.0 {
                                    id = block::STONE;
                                }

                                // Chunk should be empty by default, so we ignore if air.
                                if id != block::AIR {
                                    chunk.set_block(IVec3::new(x as i32, y as i32, z as i32), id, 0);
                                }

                                a00 += b0a0;

                            }

                            a0 += ca;
                            b0 += db;

                        }

                        a += e;
                        b += f;
                        c += g;
                        d += h;

                    }

                }
            }
        }

    }

    /// Generate the primitive terrain of the chunk.
    fn gen_surface(&mut self, cx: i32, cz: i32, chunk: &mut Chunk) {

        let sand = &mut self.cache.sand;
        let gravel = &mut self.cache.gravel;
        let thickness = &mut self.cache.thickness;

        let offset = DVec3::new((cx * 16) as f64, (cz * 16) as f64, 0.0);
        let scale = 1.0 / 32.0;
        let sea_level = 64;

        self.sand_gravel_noise.gen_3d(sand, offset, DVec3::new(scale, scale, 1.0));
        self.sand_gravel_noise.gen_2d(gravel, offset.xz(), DVec2::new(scale, scale));
        self.thickness_noise.gen_3d(thickness, offset, DVec3::splat(scale * 2.0));

        // NOTE: Order of iteration is really important for random parity.
        for z in 0usize..16 {
            for x in 0usize..16 {

                let mut pos = IVec3::new(x as i32, 0, z as i32);

                let biome = chunk.get_biome(pos);
                let have_sand = sand.get(x, z, 0) + self.rand.next_double() * 0.2 > 0.0;
                let have_gravel = gravel.get(x, 0, z) + self.rand.next_double() * 0.2 > 3.0;
                let thickness = (thickness.get(x, z, 0) / 3.0 + 3.0 + self.rand.next_double() * 0.25) as i32;

                let (
                    biome_top_id, 
                    biome_filler_id
                ) = match biome {
                    Biome::Desert |
                    Biome::IceDesert => (block::SAND, block::SAND),
                    _ => (block::GRASS, block::DIRT),
                };

                let mut top_id = biome_top_id;
                let mut filler_id = biome_filler_id;
                let mut remaining_thickness = -1;

                for y in (0..128).rev() {

                    pos.y = y;

                    if y <= self.rand.next_int_bounded(5) {
                        chunk.set_block(pos, block::BEDROCK, 0);
                        continue;
                    }

                    let (prev_id, _) = chunk.get_block(pos);

                    if prev_id == block::AIR {
                        remaining_thickness = -1;
                    } else if prev_id == block::STONE {

                        if remaining_thickness == -1 {
                            
                            // No surface yet, initialize it.
                            if thickness <= 0 {
                                top_id = block::AIR;
                                filler_id = block::STONE;
                            } else if y >= sea_level - 4 && y <= sea_level + 1 {
                                
                                top_id = biome_top_id;
                                filler_id = biome_filler_id;

                                if have_sand {
                                    top_id = block::SAND;
                                    filler_id = block::SAND;
                                } else if have_gravel {
                                    top_id = block::AIR;
                                    filler_id = block::GRAVEL;
                                }

                            }

                            if y < sea_level && top_id == block::AIR {
                                top_id = block::WATER_STILL;
                            }

                            remaining_thickness = thickness;

                            if y >= sea_level - 1 {
                                chunk.set_block(pos, top_id, 0);
                            } else {
                                chunk.set_block(pos, filler_id, 0);
                            }

                        } else if remaining_thickness > 0 {

                            chunk.set_block(pos, filler_id, 0);

                            remaining_thickness -= 1;
                            if remaining_thickness == 0 && filler_id == block::SAND {
                                remaining_thickness = self.rand.next_int_bounded(4);
                                filler_id = block::SANDSTONE;
                            }

                        }

                    }

                }

            }
        }

    }

    // Generate chunk carving (only caves for beta 1.7.3).
    fn gen_carving(&mut self, cx: i32, cz: i32, chunk: &mut Chunk) {
        self.cave_gen.generate(cx, cz, chunk);
    }

}

impl ChunkGenerator for OverworldGenerator {

    fn generate(&mut self, cx: i32, cz: i32, chunk: &mut Chunk) {

        let chunk_seed = i64::wrapping_add(
            (cx as i64).wrapping_mul(341873128712), 
            (cz as i64).wrapping_mul(132897987541));
        
        self.rand.set_seed(chunk_seed);

        self.gen_biomes(cx, cz, chunk);
        self.gen_terrain(cx, cz, chunk);
        self.gen_surface(cx, cz, chunk);
        self.gen_carving(cx, cz, chunk);

    }

}
