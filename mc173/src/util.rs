//! Various uncategorized utilities.


/// A function to better inline the default function call.
#[inline(always)]
pub(crate) fn default<T: Default>() -> T {
    T::default()
}


/// A fading average
#[derive(Debug, Clone, Default)]
pub struct FadingAverage {
    value: f32,
}

impl FadingAverage {

    #[inline]
    pub fn push(&mut self, value: f32, factor: f32) {
        self.value = (self.value * (1.0 - factor)) + value * factor;
    }

    #[inline]
    pub fn get(&self) -> f32 {
        self.value
    }

}


/// Internal utility function to split a string at a given byte index, but while keeping
/// utf8 boundary and not panicking like [`str::split_at`]. A value greater than `s.len()`
/// will panic.
#[inline]
pub fn split_at_utf8_boundary(s: &str, mut index: usize) -> (&str, &str) {
    while !s.is_char_boundary(index) {
        // Index 0 is a boundary, so we can decrement without checking overflow.
        index -= 1;
    }
    s.split_at(index)
}
