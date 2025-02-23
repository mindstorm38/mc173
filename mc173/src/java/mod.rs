//! Modules related to emulation of Java classes.

mod io;
mod rand;

pub use io::{ReadJavaExt, WriteJavaExt};
pub use rand::JavaRandom;
