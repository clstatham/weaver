//! A vector kept in sorted order.

/// A `Vec` that keeps its elements sorted, using binary search for lookups.
///
/// Duplicates are allowed. Elements can be mutated through [`SortedVec::iter_mut`], so don't
/// change anything in a way that alters its ordering.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct SortedVec<T: Ord> {
    data: Vec<T>,
}

impl<T: Ord> SortedVec<T> {
    /// Creates an empty vector.
    pub fn new() -> Self {
        Self::default()
    }

    /// Inserts `value` at its sorted position.
    pub fn insert(&mut self, value: T) {
        let index = self.data.binary_search(&value).unwrap_or_else(|x| x);
        self.data.insert(index, value);
    }

    /// Removes and returns one element equal to `value`, if there is one.
    pub fn remove(&mut self, value: &T) -> Option<T> {
        let index = self.data.binary_search(value).ok()?;
        Some(self.data.remove(index))
    }

    /// Returns whether an element equal to `value` is present.
    pub fn contains(&self, value: &T) -> bool {
        self.data.binary_search(value).is_ok()
    }

    /// Iterates in ascending order.
    pub fn iter(&self) -> impl Iterator<Item = &T> {
        self.data.iter()
    }

    /// Iterates mutably in ascending order.
    pub fn iter_mut(&mut self) -> impl Iterator<Item = &mut T> {
        self.data.iter_mut()
    }

    /// Number of elements.
    pub fn len(&self) -> usize {
        self.data.len()
    }

    /// Returns whether there are no elements.
    pub fn is_empty(&self) -> bool {
        self.data.is_empty()
    }

    /// Returns the sorted elements as a plain `Vec`.
    pub fn into_vec(self) -> Vec<T> {
        self.data
    }
}

impl<T: Ord> Default for SortedVec<T> {
    fn default() -> Self {
        Self { data: Vec::new() }
    }
}

impl<T: Ord> FromIterator<T> for SortedVec<T> {
    fn from_iter<I: IntoIterator<Item = T>>(iter: I) -> Self {
        let mut data = Vec::from_iter(iter);
        data.sort_unstable();
        Self { data }
    }
}
