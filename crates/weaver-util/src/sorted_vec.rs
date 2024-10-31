#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct SortedVec<T: Ord> {
    data: Vec<T>,
}

impl<T: Ord> SortedVec<T> {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn insert(&mut self, value: T) {
        let index = self.data.binary_search(&value).unwrap_or_else(|x| x);
        self.data.insert(index, value);
    }

    pub fn remove(&mut self, value: &T) -> Option<T> {
        let index = self.data.binary_search(value).ok()?;
        Some(self.data.remove(index))
    }

    pub fn contains(&self, value: &T) -> bool {
        self.data.binary_search(value).is_ok()
    }

    pub fn iter(&self) -> impl Iterator<Item = &T> {
        self.data.iter()
    }

    pub fn iter_mut(&mut self) -> impl Iterator<Item = &mut T> {
        self.data.iter_mut()
    }

    pub fn len(&self) -> usize {
        self.data.len()
    }

    pub fn is_empty(&self) -> bool {
        self.data.is_empty()
    }

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
