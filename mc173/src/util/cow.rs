//! A custom [`Arc`]-based Clone-On-Write wrapper.

use std::ops::{Deref, DerefMut};
use std::sync::Arc;


/// A wrapper type that provides a Clone-On-Write wrapper around an Atomically 
/// Reference-Counted value ([`Arc`]). This wrapper basically allows mutation when the
/// internal Arc has only one reference in that wrapper, but when the Arc has been 
/// strongly cloned elsewhere, then the value will be cloned to replace the current Arc,
/// then it will be guaranteed that only one reference exists, so the mutation can happen
/// transparently.
pub struct CowArc<T: ?Sized> {
    inner: Arc<T>,
}

impl<T: ?Sized> CowArc<T> {

    /// Construct a new Clone-On-Write wrapper for the given Arc.
    #[inline]
    pub fn new(inner: Arc<T>) -> Self {
        Self { inner }
    }

    /// Get a reference to the inner Arc of this Clone-On-Write wrapper.
    #[inline]
    pub fn as_inner(&self) -> &Arc<T> {
        &self.inner
    }

    /// Return a clone of the inner Arc of this Clone-On-Write wrapper.
    #[inline]
    pub fn clone_inner(&self) -> Arc<T> {
        Arc::clone(&self.inner)
    }

    /// Destruct this Clone-On-Write wrapper into the inner Arc. 
    #[inline]
    pub fn into_inner(self) -> Arc<T> {
        self.inner
    }

}

impl<T: ?Sized> Deref for CowArc<T> {

    type Target = T;

    #[inline]
    fn deref(&self) -> &Self::Target {
        &*self.inner
    }

}

impl<T: ?Sized + Clone> DerefMut for CowArc<T> {

    fn deref_mut(&mut self) -> &mut Self::Target {
        Arc::make_mut(&mut self.inner)
    }

}
