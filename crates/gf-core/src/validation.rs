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

    /// Creates a failed validation from a non-empty list, or `Valid(())` when empty.
    #[must_use]
    pub fn from_errors(errors: Vec<E>) -> Validation<(), E> {
        if errors.is_empty() {
            Validation::Valid(())
        } else {
            Validation::Invalid(errors)
        }
    }

    /// Accumulates an iterator of unit validations without short-circuiting.
    #[must_use]
    pub fn collect_all<I: IntoIterator<Item = Validation<(), E>>>(checks: I) -> Validation<(), E> {
        checks
            .into_iter()
            .fold(Validation::Valid(()), |acc, check| {
                acc.zip_with(check, |(), ()| ())
            })
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
