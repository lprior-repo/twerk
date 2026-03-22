//! Slice manipulation utilities following functional-rust conventions.
//!
//! # Architecture
//!
//! - **Data**: Plain slices as input/output
//! - **Calc**: Pure iterator-based transformations
//! - **Actions**: None - all functions are pure

use std::collections::HashSet;

/// Checks if two slices have any common elements.
///
/// # Arguments
///
/// * `a` - First slice
/// * `b` - Second slice
///
/// # Returns
///
/// `true` if any element appears in both slices, `false` otherwise.
#[must_use]
pub fn intersect<T>(a: &[T], b: &[T]) -> bool
where
    T: std::hash::Hash + Eq,
{
    let elements: HashSet<&T> = a.iter().collect();
    b.iter().any(|item| elements.contains(item))
}

/// Maps a slice of type T to a slice of type U using the provided function.
///
/// # Arguments
///
/// * `items` - Input slice
/// * `f` - Transformation function
///
/// # Returns
///
/// A new vector containing the transformed elements.
#[must_use]
pub fn map<T, U, F>(items: &[T], f: F) -> Vec<U>
where
    F: FnMut(&T) -> U,
{
    items.iter().map(f).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_intersect_with_common_elements() {
        let a = vec![1, 2, 3, 4, 5];
        let b = vec![4, 5, 6, 7, 8];
        assert!(intersect(&a, &b));
    }

    #[test]
    fn test_intersect_without_common_elements() {
        let a = vec![1, 2, 3, 4, 5];
        let b = vec![6, 7, 8, 9, 10];
        assert!(!intersect(&a, &b));
    }

    #[test]
    fn test_intersect_with_empty_first() {
        let a: Vec<i32> = vec![];
        let b = vec![1, 2, 3];
        assert!(!intersect(&a, &b));
    }

    #[test]
    fn test_intersect_with_empty_second() {
        let a = vec![1, 2, 3];
        let b: Vec<i32> = vec![];
        assert!(!intersect(&a, &b));
    }

    #[test]
    fn test_intersect_both_empty() {
        let a: Vec<i32> = vec![];
        let b: Vec<i32> = vec![];
        assert!(!intersect(&a, &b));
    }

    #[test]
    fn test_intersect_single_common() {
        let a = vec![1, 2, 3];
        let b = vec![3, 4, 5];
        assert!(intersect(&a, &b));
    }

    #[test]
    fn test_map_transform() {
        let tasks = vec!["a".to_string(), "b".to_string(), "c".to_string()];

        let names: Vec<String> = map(&tasks, |t| t.clone());

        assert_eq!(vec!["a", "b", "c"], names);
    }

    #[test]
    fn test_map_with_i32() {
        let nums = vec![1, 2, 3, 4, 5];

        let doubled: Vec<i32> = map(&nums, |&n| n * 2);

        assert_eq!(vec![2, 4, 6, 8, 10], doubled);
    }
}
