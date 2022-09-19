use std::sync::{Arc, Mutex};

use crate::{NestedBars, NestedMeta, ProgressBar, ProgressBarState};

/// Helper for spliting progress bars
pub struct ProgressBarWeightedNester {
    pub(crate) bar: ProgressBar,
    pub(crate) taken_fraction: f64,
}

impl ProgressBarWeightedNester {
    /// Adds a new child progress bar, representing a fraction of the parent.
    ///
    /// Normally the total fraction of all child bars that you add should sum up to 1.0.
    /// If you exceed 1.0, the fractions will be normalized so that they still sum up to 1.0.
    pub fn take(&mut self, fraction_of_total: f64) -> ProgressBar {
        assert!(fraction_of_total.is_finite());
        assert!(
            fraction_of_total >= 0.0,
            "fraction_of_total must be non-negative"
        );
        assert!(
            fraction_of_total <= 1.0,
            "fraction_of_total must be at most 1.0"
        );

        let s = Arc::new(Mutex::new(ProgressBarState::default()));
        if let Some(NestedBars {
            bars,
            meta: NestedMeta::Weighted(weights),
        }) = &mut self.bar.state.as_ref().unwrap().lock().unwrap().nested
        {
            bars.push(s.clone());
            weights.push(fraction_of_total);
        } else {
            unreachable!();
        }

        self.taken_fraction += fraction_of_total;
        ProgressBar { state: Some(s) }
    }

    /// Adds a new child progress bar, representing the remaining fraction of the parent bar.
    pub fn remaining(&mut self) -> ProgressBar {
        // Check if the whole progress bar has been used up already.
        // We guard against small floating point errors by not being so strict with this check.
        if self.taken_fraction > 1.01 {
            panic!(
                "There is no remaning part of the progress bar. You have already used {}% of it",
                self.taken_fraction * 100.0
            );
        }

        self.take((1.0 - self.taken_fraction).max(0.0))
    }
}

/// Helper for spliting progress bars
pub struct ProgressBarSizedNester {
    pub(crate) bar: ProgressBar,
    pub(crate) taken_count: usize,
}

impl ProgressBarSizedNester {
    /// Adds a new child progress bar, representing `count` items, to the parent.
    ///
    /// The child bar will have its length set to `count`, but this is not strictly necessary.
    /// A full child bar will be remapped to `count` items in the parent regardless of how long the child bar actually is.
    pub fn take(&mut self, count: usize) -> ProgressBar {
        let s = Arc::new(Mutex::new(ProgressBarState {
            length: Some(count),
            ..Default::default()
        }));
        if let Some(NestedBars {
            bars,
            meta: NestedMeta::Sized(counts),
        }) = &mut self.bar.state.as_ref().unwrap().lock().unwrap().nested
        {
            bars.push(s.clone());
            counts.push(count as f64);
        } else {
            unreachable!();
        }

        self.taken_count += count;
        ProgressBar { state: Some(s) }
    }

    /// Adds a new child progress bar, representing the remaining items in the parent bar.
    ///
    /// This method only works if the parent bar has a length set. Otherwise this function will panic.
    pub fn remaining(&mut self) -> ProgressBar {
        let len = self.bar.state.as_ref().unwrap().lock().unwrap().length;
        match len {
            Some(len) => {
                if let Some(remaining) = len.checked_sub(self.taken_count) {
                    self.take(remaining)
                } else {
                    panic!(
                        "There is no remaning part of the progress bar. The bar has a length of {} and you have already used {} for other nested bars.",
                        len,
                        self.taken_count
                    );
                }
            }
            None => {
                panic!(
                    "You cannot call remaining because the original bar didn't have a length set"
                );
            }
        }
    }
}

/// Helper for spliting progress bars
pub struct ProgressBarSummedNester {
    pub(crate) bar: ProgressBar,
}

impl ProgressBarSummedNester {
    /// Adds a new child progress bar to the parent.
    ///
    /// The parent will display the sum of all children's progress and lengths.
    pub fn take(&self) -> ProgressBar {
        let s = Arc::new(Mutex::new(ProgressBarState::default()));
        if let Some(NestedBars {
            bars,
            meta: NestedMeta::Summed,
        }) = &mut self.bar.state.as_ref().unwrap().lock().unwrap().nested
        {
            bars.push(s.clone());
        } else {
            unreachable!();
        }

        ProgressBar { state: Some(s) }
    }
}
