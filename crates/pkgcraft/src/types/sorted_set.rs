use std::cmp::Ordering;
use std::fmt::Debug;
use std::hash::{Hash, Hasher};
use std::ops::{Deref, DerefMut};

use indexmap::IndexSet;
use itertools::EitherOrBoth::{Both, Left, Right};
use itertools::Itertools;
use serde::{Deserialize, Deserializer};

use crate::macros::partial_cmp_not_equal_opt;

use super::{make_set_traits, Ordered};

/// Wrapper for IndexSet that implements Ord and Hash via sorting.
#[derive(Debug, Clone)]
pub struct SortedSet<T: Ordered>(IndexSet<T>);

impl<T: Ordered> Default for SortedSet<T> {
    fn default() -> Self {
        Self(Default::default())
    }
}

impl<T: Ordered> From<IndexSet<T>> for SortedSet<T> {
    fn from(value: IndexSet<T>) -> Self {
        Self(value)
    }
}

impl<T: Ordered> SortedSet<T> {
    /// Construct a new, empty SortedSet<T>.
    pub fn new() -> Self {
        Self::default()
    }
}

impl<T: Ordered> Hash for SortedSet<T> {
    fn hash<H: Hasher>(&self, state: &mut H) {
        for e in self.0.iter().sorted() {
            e.hash(state);
        }
    }
}

impl<T: Ordered> Ord for SortedSet<T> {
    fn cmp(&self, other: &Self) -> Ordering {
        self.0.iter().sorted().cmp(other.0.iter().sorted())
    }
}

impl<T1, T2> PartialOrd<SortedSet<T1>> for SortedSet<T2>
where
    T1: Ordered,
    T2: Ordered + PartialOrd<T1>,
{
    fn partial_cmp(&self, other: &SortedSet<T1>) -> Option<Ordering> {
        for item in self.iter().sorted().zip_longest(other.iter().sorted()) {
            match item {
                Both(v1, v2) => partial_cmp_not_equal_opt!(v1, v2),
                Left(_) => return Some(Ordering::Greater),
                Right(_) => return Some(Ordering::Less),
            }
        }
        Some(Ordering::Equal)
    }
}

impl<T1, T2> PartialEq<SortedSet<T1>> for SortedSet<T2>
where
    T1: Ordered,
    T2: Ordered + PartialOrd<T1>,
{
    fn eq(&self, other: &SortedSet<T1>) -> bool {
        self.partial_cmp(other) == Some(Ordering::Equal)
    }
}

impl<T: Ordered> Eq for SortedSet<T> {}

impl<T: Ordered> FromIterator<T> for SortedSet<T> {
    fn from_iter<I: IntoIterator<Item = T>>(iterable: I) -> Self {
        Self(iterable.into_iter().collect())
    }
}

impl<T: Ordered, const N: usize> From<[T; N]> for SortedSet<T> {
    fn from(arr: [T; N]) -> Self {
        Self::from_iter(arr)
    }
}

impl<'a, T: Ordered> IntoIterator for &'a SortedSet<T> {
    type Item = &'a T;
    type IntoIter = indexmap::set::Iter<'a, T>;

    fn into_iter(self) -> Self::IntoIter {
        self.0.iter()
    }
}

impl<T: Ordered> IntoIterator for SortedSet<T> {
    type Item = T;
    type IntoIter = indexmap::set::IntoIter<T>;

    fn into_iter(self) -> Self::IntoIter {
        self.0.into_iter()
    }
}

impl<T: Ordered> Deref for SortedSet<T> {
    type Target = IndexSet<T>;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl<T: Ordered> DerefMut for SortedSet<T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}

impl<'de, T> Deserialize<'de> for SortedSet<T>
where
    T: Ordered + Deserialize<'de>,
{
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        IndexSet::deserialize(deserializer).map(SortedSet)
    }
}

make_set_traits!(SortedSet<T>);

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hash() {
        // different elements
        let s1 = SortedSet::from(["a"]);
        let s2 = SortedSet::from(["b"]);
        assert_ne!(&s1, &s2);
        assert_ne!(SortedSet::from([s1, s2]).len(), 1);

        // different ordering
        let s1 = SortedSet::from(["a", "b"]);
        let s2 = SortedSet::from(["b", "a"]);
        assert_eq!(&s1, &s2);
        assert_eq!(SortedSet::from([s1, s2]).len(), 1);

        // similar ordering
        let s1 = SortedSet::from(["a", "b"]);
        let s2 = SortedSet::from(["a", "b"]);
        assert_eq!(&s1, &s2);
        assert_eq!(SortedSet::from([s1, s2]).len(), 1);

        // matching elements
        let s1 = SortedSet::from(["a", "b", "a"]);
        let s2 = SortedSet::from(["a", "b", "b"]);
        assert_eq!(&s1, &s2);
        assert_eq!(SortedSet::from([s1, s2]).len(), 1);
    }
}
