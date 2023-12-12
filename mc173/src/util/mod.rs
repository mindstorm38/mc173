//! Various math utilities.

mod noise;
mod rand;
mod face;
mod math;
mod bb;
mod io;

pub use rand::JavaRandom;
pub use bb::BoundingBox;

pub use face::{Face, FaceSet};

pub use io::{ReadJavaExt, WriteJavaExt};

pub use noise::{NoiseCube, PerlinNoise, PerlinOctaveNoise};

pub use math::MinecraftMath;


/// A function to better inline the default function call.
#[inline(always)]
pub(crate) fn default<T: Default>() -> T {
    T::default()
}
