//! Immutable application values with deterministic, quota-visible spare capacity.
use std::{ops::Deref, sync::Arc};

#[derive(Debug)]
struct Storage<T> {
    data: Vec<T>,
    capacity: usize,
}
impl<T: Clone> Clone for Storage<T> {
    fn clone(&self) -> Self {
        let mut data = Vec::with_capacity(self.capacity);
        data.extend_from_slice(&self.data);
        Self {
            data,
            capacity: self.capacity,
        }
    }
}
#[derive(Clone, Debug)]
pub struct Sequence<T>(Arc<Storage<T>>);
impl<T: PartialEq> PartialEq for Sequence<T> {
    fn eq(&self, other: &Self) -> bool {
        self.as_ref() == other.as_ref()
    }
}
impl<T: Eq> Eq for Sequence<T> {}
impl<T> Deref for Sequence<T> {
    type Target = [T];
    fn deref(&self) -> &[T] {
        &self.0.data
    }
}
impl<T> AsRef<[T]> for Sequence<T> {
    fn as_ref(&self) -> &[T] {
        self
    }
}
impl<T> From<Vec<T>> for Sequence<T> {
    fn from(data: Vec<T>) -> Self {
        let capacity = data.len();
        // Admission/constructors have exact semantic capacity, not a Host's
        // incidental Vec spare storage. The boxed conversion shrinks that storage.
        let data = data.into_boxed_slice().into_vec();
        Self(Arc::new(Storage { data, capacity }))
    }
}
impl<T> Sequence<T> {
    pub(crate) fn identity(&self) -> usize {
        Arc::as_ptr(&self.0) as usize
    }
    pub(crate) fn capacity(&self) -> usize {
        self.0.capacity
    }
}
impl<T: Clone> Sequence<T> {
    pub(crate) fn mutable(&mut self) -> &mut [T] {
        &mut Arc::make_mut(&mut self.0).data
    }
    pub(crate) fn concat(&self, other: &Self, capacity: usize) -> Self {
        let mut data = Vec::with_capacity(capacity);
        data.extend_from_slice(self);
        data.extend_from_slice(other);
        Self(Arc::new(Storage { data, capacity }))
    }
    pub(crate) fn append(&mut self, other: &Self, capacity: usize) {
        let storage = Arc::make_mut(&mut self.0);
        if storage.capacity != capacity {
            storage.data.reserve_exact(capacity - storage.data.len());
            storage.capacity = capacity;
        }
        storage.data.extend_from_slice(other);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn spare_capacity_and_cow_preserve_immutable_aliases() {
        let mut a: Sequence<i64> = vec![1, 2].into();
        let original = a.clone();
        a.append(&vec![3].into(), 8);
        assert_eq!(a.capacity(), 8);
        a.mutable()[0] = 9;
        assert_eq!(original.as_ref(), &[1, 2]);
        assert_eq!(a, vec![9, 2, 3].into());
        assert_eq!(a.capacity(), 8);
    }
}
