//! Slice utilities with functional style.

// ============================================================================
// Intrinsic Imports
// ============================================================================
use std::collections::HashSet;

// ============================================================================
// Public Functions
// ============================================================================

/// Returns `true` if any element from `a` exists in `b`.
///
/// # Type Parameters
/// - `T`: Must implement `PartialEq` for equality comparison and `std::hash::Hash` for Set insertion.
///
/// # Arguments
/// * `a` - First slice to check against
/// * `b` - Second slice to search within
///
/// # Examples
/// ```
/// use twerk_common::slices::intersect;
///
/// assert!(intersect(&[1, 2, 3], &[4, 3, 5]));
/// assert!(!intersect(&[1, 2, 3], &[4, 5, 6]));
/// ```
#[must_use]
pub fn intersect<T>(a: &[T], b: &[T]) -> bool
where
    T: PartialEq + std::hash::Hash + Eq,
{
    let b_set: HashSet<&T> = b.iter().collect();
    a.iter().any(|item| b_set.contains(item))
}

/// Transforms each element in `items` using the provided function `f`.
///
/// # Type Parameters
/// - `T`: Input element type
/// - `U`: Output element type produced by `f`
///
/// # Arguments
/// * `items` - Slice of items to transform
/// * `f` - Function to apply to each element
///
/// # Examples
/// ```
/// use twerk_common::slices::map_slice;
///
/// let nums = [1, 2, 3];
/// let doubled: Vec<i32> = map_slice(&nums, |x| x * 2);
/// assert_eq!(doubled, vec![2, 4, 6]);
/// ```
#[must_use]
pub fn map_slice<T, U>(items: &[T], f: impl Fn(T) -> U) -> Vec<U>
where
    T: Clone,
{
    items.iter().map(|item| f(item.clone())).collect()
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    // ========================================================================
    // Intersect Tests
    // ========================================================================

    mod intersect {
        use super::*;

        #[test]
        fn returns_true_when_element_exists_in_both() {
            assert!(intersect(&[1, 2, 3], &[4, 3, 5]));
        }

        #[test]
        fn returns_true_when_first_element_matches() {
            assert!(intersect(&[1, 2, 3], &[1, 5, 6]));
        }

        #[test]
        fn returns_true_when_last_element_matches() {
            assert!(intersect(&[1, 2, 3], &[4, 5, 3]));
        }

        #[test]
        fn returns_false_when_no_elements_match() {
            assert!(!intersect(&[1, 2, 3], &[4, 5, 6]));
        }

        #[test]
        fn returns_false_on_empty_a() {
            assert!(!intersect::<i32>(&[], &[1, 2, 3]));
        }

        #[test]
        fn returns_false_on_empty_b() {
            assert!(!intersect(&[1, 2, 3], &[]));
        }

        #[test]
        fn returns_false_when_both_empty() {
            assert!(!intersect::<i32>(&[], &[]));
        }

        #[test]
        fn works_with_strings() {
            assert!(intersect(&["apple", "banana"], &["cherry", "banana"]));
            assert!(!intersect(&["apple", "banana"], &["cherry", "date"]));
        }

        #[test]
        fn works_with_duplicates_in_a() {
            assert!(intersect(&[1, 1, 2], &[3, 1, 4]));
        }

        #[test]
        fn works_with_duplicates_in_b() {
            assert!(intersect(&[1, 2], &[3, 3, 1, 3]));
        }
    }

    // ========================================================================
    // Map Tests
    // ========================================================================

    mod map_slice {
        use super::*;

        #[test]
        fn transforms_integers() {
            let nums = [1, 2, 3];
            let result = map_slice(&nums, |x: i32| x * 2);
            assert_eq!(result, vec![2, 4, 6]);
        }

        #[test]
        fn transforms_to_different_type() {
            let nums = [1, 2, 3];
            let result: Vec<String> = map_slice(&nums, |x: i32| x.to_string());
            assert_eq!(result, vec!["1", "2", "3"]);
        }

        #[test]
        fn returns_empty_on_empty_input() {
            let nums: [i32; 0] = [];
            let result: Vec<i32> = map_slice(&nums, |x| x * 2);
            assert!(result.is_empty());
        }

        #[test]
        fn works_with_strings() {
            let words = ["hello", "world"];
            let result: Vec<String> = map_slice(&words, |s: &str| s.to_uppercase());
            assert_eq!(result, vec!["HELLO", "WORLD"]);
        }

        #[test]
        fn identity_mapping() {
            let nums = [1, 2, 3];
            let result: Vec<i32> = map_slice(&nums, |x| x);
            assert_eq!(result, vec![1, 2, 3]);
        }

        #[test]
        fn handles_single_element() {
            let nums = [42];
            let result: Vec<i32> = map_slice(&nums, |x| x + 1);
            assert_eq!(result, vec![43]);
        }

        #[test]
        fn preserves_order() {
            let nums = [5, 10, 15, 20];
            let result: Vec<i32> = map_slice(&nums, |x| x / 5);
            assert_eq!(result, vec![1, 2, 3, 4]);
        }
    }
}
