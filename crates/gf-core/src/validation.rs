//! Applicative error-accumulating `Validation<T, E>` functor.

use alloc::vec;
use alloc::vec::Vec;

/// Applicative validation result that accumulates all errors instead of short-circuiting.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Validation<T, E> {
    /// Successful value with zero accumulated errors.
    Valid(T),
    /// One or more accumulated validation errors.
    Invalid(Vec<E>),
}

impl<T, E> Validation<T, E> {
    /// Wraps a valid value.
    #[must_use]
    pub const fn valid(value: T) -> Self {
        Self::Valid(value)
    }

    /// Creates a failed validation holding a single error.
    #[must_use]
    pub fn invalid_one(error: E) -> Self {
        Self::Invalid(vec![error])
    }

    /// Returns `true` when validation succeeded.
    #[must_use]
    pub const fn is_valid(&self) -> bool {
        matches!(self, Self::Valid(_))
    }

    /// Transforms the valid value while preserving accumulated errors.
    #[must_use]
    pub fn map<U, F: FnOnce(T) -> U>(self, f: F) -> Validation<U, E> {
        match self {
            Self::Valid(value) => Validation::Valid(f(value)),
            Self::Invalid(errors) => Validation::Invalid(errors),
        }
    }

    /// Combines two independent validations, accumulating errors from both sides if invalid.
    #[must_use]
    pub fn zip_with<U, V, F: FnOnce(T, U) -> V>(
        self,
        other: Validation<U, E>,
        combine: F,
    ) -> Validation<V, E> {
        match (self, other) {
            (Self::Valid(left), Validation::Valid(right)) => {
                Validation::Valid(combine(left, right))
            }
            (Self::Invalid(mut left_errs), Validation::Invalid(right_errs)) => {
                left_errs.extend(right_errs);
                Validation::Invalid(left_errs)
            }
            (Self::Invalid(errs), Validation::Valid(_))
            | (Self::Valid(_), Validation::Invalid(errs)) => Validation::Invalid(errs),
        }
    }

    /// Converts this validation into a standard [`Result`].
    ///
    /// # Errors
    ///
    /// Returns `Err(Vec<E>)` containing every accumulated error when invalid.
    pub fn into_result(self) -> Result<T, Vec<E>> {
        match self {
            Self::Valid(value) => Ok(value),
            Self::Invalid(errors) => Err(errors),
        }
    }
}

#[cfg(test)]
mod tests {
    use alloc::vec;

    use super::Validation;

    #[test]
    fn accumulates_errors_across_independent_checks() {
        let a: Validation<u32, &str> = Validation::invalid_one("bad_a");
        let b: Validation<u32, &str> = Validation::invalid_one("bad_b");
        let combined = a.zip_with(b, u32::saturating_add);
        assert!(!combined.is_valid());
        assert_eq!(combined.into_result(), Err(vec!["bad_a", "bad_b"]));
    }

    #[test]
    fn maps_and_combines_valid_values() {
        let left: Validation<u32, &str> = Validation::valid(20).map(|v| v + 2);
        let right: Validation<u32, &str> = Validation::valid(20);
        let merged = left.zip_with(right, u32::saturating_add);
        assert!(merged.is_valid());
        assert_eq!(merged.into_result(), Ok(42));

        let mixed = Validation::<u32, &str>::valid(1).zip_with(
            Validation::<u32, &str>::invalid_one("only_right"),
            u32::saturating_add,
        );
        assert_eq!(mixed.into_result(), Err(vec!["only_right"]));
    }
}
