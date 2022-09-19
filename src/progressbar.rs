use std::sync::{Arc, Mutex};
use std::thread;

use crate::{
    manager_thread, LifecycleState, NestedBars, NestedMeta, ProgressBarSizedNester,
    ProgressBarSummedNester, MANAGER,
};
use crate::{ProgressBarState, ProgressBarWeightedNester};

pub struct ProgressBar {
    pub(crate) state: Option<Arc<Mutex<ProgressBarState>>>,
}

impl Drop for ProgressBar {
    fn drop(&mut self) {
        self.abandon();
    }
}

impl Default for ProgressBar {
    fn default() -> Self {
        Self::new()
    }
}

/// A convenient progress bar.
///
/// See the [module documentation](crate) for example code and more documentation.
#[doc=include_str!("../images/simple.html")]
impl ProgressBar {
    /// Creates a new progress bar.
    ///
    /// ```
    /// use headway::ProgressBar;
    /// # use std::time::Duration;
    /// # use std::thread::sleep;
    ///
    /// let p = ProgressBar::new().with_message("Calibrating flux capacitors");
    /// for _ in p.wrap(0..100) {
    ///     sleep(Duration::from_millis(20));
    /// }
    /// ```
    #[doc=include_str!("../images/message.html")]
    pub fn new() -> Self {
        let mut manager = MANAGER.lock().unwrap();
        let state = Arc::new(Mutex::new(ProgressBarState::default()));
        manager.bars.push(state.clone());
        if manager.interactive_output && !manager.thread_started {
            manager.thread_started = true;
            thread::spawn(manager_thread);
        }
        Self { state: Some(state) }
    }

    /// Creates a new progress bar which will never be rendered.
    ///
    /// This can be useful if you need to pass a progress bar to some function, but you don't actually want a bar to show up.
    pub fn hidden() -> Self {
        let state = Arc::new(Mutex::new(ProgressBarState::default()));
        Self { state: Some(state) }
    }

    /// Splits the bar into children of given proportions.
    ///
    /// This is useful if you have many tasks, but you only want to show a single progress bar.
    /// Then you can split the bar so that for example the first task gets 40% of the bar and the second task gets 60% of the bar.
    ///
    /// The display will automatically change to percentages if this mode of splitting is used.
    ///
    /// ```
    /// # use std::time::Duration;
    /// # use std::thread::sleep;
    /// use headway::{ProgressBar, ProgressBarIterable};
    ///
    /// let mut p = ProgressBar::new().split_weighted();
    /// let first_half = p.take(0.4).with_message("First part");
    /// let second_half = p.take(0.6).with_message("Second part");
    /// for _ in (0..50).progress_with(first_half) {
    ///     sleep(Duration::from_millis(20));
    /// }
    /// for _ in (0..50).progress_with(second_half) {
    ///     sleep(Duration::from_millis(30));
    /// }
    /// ```
    #[doc=include_str!("../images/split_weighted.html")]
    pub fn split_weighted(self) -> ProgressBarWeightedNester {
        self.state
            .as_ref()
            .expect("You cannot split a finished/abandoned progress bar")
            .lock()
            .unwrap()
            .nested = Some(NestedBars {
            bars: vec![],
            meta: NestedMeta::Weighted(vec![]),
        });
        ProgressBarWeightedNester {
            bar: self,
            taken_fraction: 0.0,
        }
    }

    /// Splits the bar into children of fixed sizes.
    ///
    /// Each child bar will represent N items of the parent.
    /// Note that the children can have any length, but a filled child bar will be remapped to N items in the parent.
    ///
    /// ```
    /// # use std::time::Duration;
    /// # use std::thread::sleep;
    /// use headway::ProgressBar;
    ///
    /// let mut p = ProgressBar::new().split_sized();
    /// // Create the bars up front so that the bar knows how many items
    /// // there are in total.
    /// let first = p.take(5).with_message("First");
    /// let second = p.take(20).with_message("Second");
    ///
    /// for _ in first.wrap(0..5) {
    ///     sleep(Duration::from_millis(300));
    /// }
    ///
    /// // Here we only loop over 5 items, but we make the child bar represent
    /// // 20 items in the parent bar.
    /// for _ in second.wrap(0..5) {
    ///     sleep(Duration::from_millis(300));
    /// }
    /// ```
    #[doc=include_str!("../images/split_sized.html")]
    pub fn split_sized(self) -> ProgressBarSizedNester {
        self.state
            .as_ref()
            .expect("You cannot split a finished/abandoned progress bar")
            .lock()
            .unwrap()
            .nested = Some(NestedBars {
            bars: vec![],
            meta: NestedMeta::Sized(vec![]),
        });
        ProgressBarSizedNester {
            bar: self,
            taken_count: 0,
        }
    }

    /// Splits the bar into children and displays the summed progress of all children.
    ///
    /// ```
    /// # use std::time::Duration;
    /// # use std::thread::sleep;
    /// use headway::ProgressBar;
    ///
    /// let p = ProgressBar::new().split_summed();
    /// let tasks = (0..4)
    ///     .map(|_| {
    ///         let child_bar = p.take();
    ///         std::thread::spawn(move || {
    ///             for _ in child_bar.wrap(0..100) {
    ///                 sleep(Duration::from_millis(20));
    ///             }
    ///         })
    ///     })
    ///     .collect::<Vec<_>>();
    /// for task in tasks {
    ///     task.join().unwrap()
    /// }
    /// ```
    #[doc=include_str!("../images/split_summed.html")]
    pub fn split_summed(self) -> ProgressBarSummedNester {
        self.state
            .as_ref()
            .expect("You cannot split a finished/abandoned progress bar")
            .lock()
            .unwrap()
            .nested = Some(NestedBars {
            bars: vec![],
            meta: NestedMeta::Summed,
        });
        ProgressBarSummedNester { bar: self }
    }

    /// Splits the bar into N bars, each representing an item in the iterator.
    ///
    /// This is useful if each item takes a long time and you want progress for it.
    /// It is also useful if you are sending each item to separate threads to process them independently.
    /// ```
    /// # use std::time::Duration;
    /// # use std::thread::sleep;
    /// use headway::ProgressBar;
    ///
    /// let p = ProgressBar::new();
    /// // Split the progress bar into 10 nested bars
    /// for (nested_bar, i) in p.split_each(0..10) {
    ///     // Wrap the nested bar around an iterator representing this subtask
    ///     nested_bar.set_message(format!("Subtask {}", i));
    ///     for _ in nested_bar.wrap(0..200) {
    ///         sleep(Duration::from_millis(5));
    ///     }
    /// }
    /// ```
    #[doc=include_str!("../images/split_each.html")]
    pub fn split_each<It: Iterator>(self, it: It) -> impl Iterator<Item = (ProgressBar, It::Item)> {
        if let Some(upper_bound) = it.size_hint().1 {
            self.set_length(upper_bound);
        }
        let mut splitter = self.split_sized();
        it.map(move |v| (splitter.take(1), v))
    }

    /// Length of the bar, if it has been set
    pub fn length(&self) -> Option<usize> {
        if let Some(state) = &self.state {
            state.lock().unwrap().length
        } else {
            panic!(
                "This progress bar is finished. You can no longer retrieve information about it."
            );
        }
    }

    /// Sets the length of this progress bar.
    ///
    /// This has no effect if the bar has already been finished or abandoned.
    pub fn set_length(&self, len: usize) {
        if let Some(state) = &self.state {
            let mut state = state.lock().unwrap();
            state.length = Some(len);
        }
    }

    /// Sets the amount of progress this bar has made.
    ///
    /// Should usually be between 0 and [`Self::length`].
    ///
    /// This has no effect if the bar has already been finished or abandoned.
    pub fn set_position(&self, pos: usize) {
        if let Some(state) = &self.state {
            state.lock().unwrap().position = pos;
        }
    }

    /// Clears any message set using [`Self::set_message`] or [`Self::with_message`].
    pub fn clear_message(&self) {
        if let Some(state) = &self.state {
            state.lock().unwrap().message = None;
        }
    }

    /// Equivalent to [`Self::set_message`], but may be more ergonomic in some situations since it returns `self`.
    pub fn with_message(self, message: impl Into<String>) -> Self {
        self.set_message(message);
        self
    }

    /// Equivalent to [`Self::set_length`], but may be more ergonomic in some situations since it returns `self`.
    pub fn with_length(self, length: usize) -> Self {
        self.set_length(length);
        self
    }

    /// Sets a message which will show up next to the bar.
    ///
    /// If the root bar has been split into multiple children, then the message that is displayed
    /// is from the first bar that is not finished. Or if all bars are finished then the last bar with a message will be used.
    pub fn set_message(&self, message: impl Into<String>) {
        let m = message.into();
        if m.is_empty() {
            self.clear_message();
        } else if let Some(state) = &self.state {
            state.lock().unwrap().message = Some(m);
        }
    }

    /// Increments the progress of this bar by 1.
    ///
    /// Usually it's more convenient to work with the iterator-wrapping functions like [`Self::wrap`]
    pub fn inc(&self) {
        if let Some(state) = &self.state {
            state.lock().unwrap().position += 1;
        }
    }

    /// Marks the bar as finished and sets the message.
    ///
    /// Equivalent to first setting the message and then marking the bar as finished.
    pub fn finish_with_message(&mut self, message: impl Into<String>) {
        self.set_message(message);
        self.finish();
    }

    /// Abandons the progress bar.
    ///
    /// The remaining part of the progress bar will be colored red to indicate it will never be completed.
    /// Progress bars are automatically marked as abandoned when they are dropped and they are only partially complete.
    pub fn abandon(&mut self) {
        if let Some(state) = &self.state {
            let mut state = state.lock().unwrap();
            state.lifecycle = LifecycleState::Abandoned;
        }
        self.state = None;

        let mut manager = MANAGER.lock().unwrap();
        manager.tick(&mut std::io::stdout().lock()).unwrap();
    }

    /// Marks the bar as finished.
    ///
    /// If the bar has a length, the position of the bar will be set to [`Self::length`].
    pub fn finish(&mut self) {
        if let Some(state) = &self.state {
            let mut state = state.lock().unwrap();
            if let Some(length) = state.length {
                state.position = length;
            }
            state.lifecycle = LifecycleState::Completed;
        }
        self.state = None;

        let mut manager = MANAGER.lock().unwrap();
        manager.tick(&mut std::io::stdout().lock()).unwrap();
    }

    /// Wraps the bar around an iterator.
    ///
    /// If the iterator has a known length, the bar's length will be set to that length.
    /// The iterator will headway the progress by 1 each step.
    /// When reaching the end of the iterator, the bar will be marked as finished.
    ///
    /// See also [`ProgressBarIterable::progress`] and [`ProgressBarIterable::progress_with`]
    ///
    /// ```
    /// # use std::time::Duration;
    /// # use std::thread::sleep;
    /// use headway::ProgressBar;
    ///
    /// let p = ProgressBar::new().with_message("Calibrating flux capacitors");
    /// for _ in p.wrap(0..100) {
    ///     sleep(Duration::from_millis(20));
    /// }
    /// ```
    ///
    #[doc=include_str!("../images/message.html")]
    pub fn wrap<It: Iterator>(self, it: It) -> ProgressBarIterator<It> {
        if let Some(upper_bound) = it.size_hint().1 {
            self.set_length(upper_bound);
        }
        ProgressBarIterator {
            progress: self,
            inner: it,
        }
    }
}

pub struct ProgressBarIterator<It: Iterator> {
    progress: ProgressBar,
    inner: It,
}

impl<It: Iterator> Iterator for ProgressBarIterator<It> {
    type Item = It::Item;

    fn next(&mut self) -> Option<It::Item> {
        let r = self.inner.next();
        if r.is_none() {
            self.progress.finish();
        } else {
            self.progress.inc();
        }
        r
    }
}

impl<T, It: ExactSizeIterator<Item = T>> ExactSizeIterator for ProgressBarIterator<It> {
    fn len(&self) -> usize {
        self.inner.len()
    }
}

pub trait ProgressBarIterable: Iterator + Sized {
    /// Show a progress bar while iterating.
    ///
    /// The returned iterator yields the same items as the original.
    ///
    /// ```
    /// use headway::ProgressBarIterable;
    /// # use std::time::Duration;
    /// # use std::thread::sleep;
    /// for _ in (0..100).progress() {
    ///     sleep(Duration::from_millis(20));
    /// }
    /// ```
    #[doc=include_str!("../images/simple.html")]
    fn progress(self) -> ProgressBarIterator<Self>;
    /// Show a progress bar while iterating.
    ///
    /// The returned iterator yields the same items as the original.
    ///
    /// Will override the length of the progress bar if the iterator has a known length.
    /// When the iterator finishes, the bar will be marked as finished.
    ///
    /// This is equivalent to using [`ProgressBar::wrap`], but this function may be more ergonomic in some situations.
    fn progress_with(self, bar: ProgressBar) -> ProgressBarIterator<Self>;
}

impl<T, It: Iterator<Item = T>> ProgressBarIterable for It {
    fn progress(self) -> ProgressBarIterator<It> {
        self.progress_with(ProgressBar::new())
    }

    fn progress_with(self, bar: ProgressBar) -> ProgressBarIterator<It> {
        bar.wrap(self)
    }
}
