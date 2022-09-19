//! A progress bar library with a focus on ergonomics.
//!
//! # Usage
//!
//! ```
//! use headway::ProgressBarIterable;
//! # use std::time::Duration;
//! # use std::thread::sleep;
//!
//! for _ in (0..100).progress() {
//!     sleep(Duration::from_millis(20));
//! }
//! ```
#![doc=include_str!("../images/simple.html")]
//!
//! ## Multiple bars
//!
//! Multiple bars are transparently supported. Just create more of them and they will automatically
//! be placed so that they do not overlap.
//!
//! ```
//! use headway::ProgressBarIterable;
//! # use std::time::Duration;
//! # use std::thread::sleep;
//! # use std::thread;
//!
//! let mut handles = vec![];
//! for i in 0..5 {
//!     handles.push(thread::spawn(move || {
//!         for _ in (0..100).progress() {
//!             sleep(Duration::from_millis(20 + i * 20));
//!         }
//!     }));
//! }
//! for handle in handles {
//!     handle.join().unwrap();
//! }
//! ```
#![doc=include_str!("../images/multiple.html")]
//!
//! ## Splitting bars
//!
//! You can split bars into smaller bars if you have a task that consists of several sub-tasks.
//!
//! ```
//! use headway::{ProgressBar, ProgressBarIterable};
//! # use std::time::Duration;
//! # use std::thread::sleep;
//!
//! let mut p = ProgressBar::new().split_weighted();
//! let first_half = p.take(0.5).with_message("First part");
//! let second_half = p.take(0.5).with_message("Second part");
//! for _ in (0..50).progress_with(first_half) {
//!     sleep(Duration::from_millis(20));
//! }
//! for _ in (0..50).progress_with(second_half) {
//!     sleep(Duration::from_millis(30));
//! }
//! ```
#![doc=include_str!("../images/split_weighted.html")]
//!
//! You can also split in other ways, not just using fractions.
//!
//! ```
//! # use std::time::Duration;
//! # use std::thread::sleep;
//! use headway::ProgressBar;
//!
//! // Split the bar into bars taking up a fixed fraction of the parent
//! let mut p = ProgressBar::new().split_weighted();
//! let first_quarter = p.take(0.25);
//! let last_three_quarters = p.take(0.75);
//!
//! // Split the bar into fixed size nested bars
//! let p = ProgressBar::new();
//! p.set_length(50);
//! let mut p = p.split_sized();
//! let first_10 = p.take(10);
//! let another_30 = p.take(30);
//! let last_10 = p.remaining();
//!
//! // Split the bar and display it by summing the progress from each child
//! let p = ProgressBar::new().split_summed();
//! let first = p.take();
//! let second = p.take();
//!
//! // Split into several bars, each representing one item of the iterator
//! let items = &["a", "b", "c", "d"];
//! for (nested_bar, letter) in ProgressBar::new().split_each(items.iter()) {}
//! ```
//!
//! ## Printing while a progress bar is visible
//!
//! Most progress bar libraries have their output messed up in some way if you try to e.g. call `println` while a progress bar is visible.
//! Either the progress bar gets clobbered, or your printed text gets overwritten, or both!
//!
//! This library interacts properly with stdout so you can freely use `println` while a progress bar (or multiple) is visible.
//! ```
//! use headway::ProgressBarIterable;
//! # use std::time::Duration;
//! # use std::thread::sleep;
//!
//! for i in (0..100).progress() {
//!     if i % 10 == 0 {
//!         println!("{}", i);
//!     }
//!     sleep(Duration::from_millis(20));
//! }
//! ```
#![doc=include_str!("../images/print_during_progress.html")]
//!
//! ### Caveats
//! Printing to `stderr` has the potential to mess things up. However, if you flush `stdout` before you print to `stderr` then things should work properly.
//! If a child process prints to `stdout`, this also has the potential to mess things up.
//!
//! ## Abandoning bars
//!
//! If you abandon a bar without finishing it (for example because a worker thread crashed), then the bar
//! will draw angry red marks to draw your attention. You can also explicitly abandon a bar using [`ProgressBar::abandon`].
//!
//! ```should_panic
//! use headway::ProgressBarIterable;
//! # use std::time::Duration;
//! # use std::thread::sleep;
//!
//! for i in (0..100).progress() {
//!     if i == 20 {
//!         panic!("Something went wrong!");
//!     }
//!     sleep(Duration::from_millis(50));
//! }
//! ```
#![doc=include_str!("../images/abandonment.html")]
//!
//! ## Indeterminate bars
//!
//! If the progress bar doesn't have a known length, the bar will show an animation instead.
//!
//! ```
//! use headway::ProgressBarIterable;
//! # use std::time::Duration;
//! # use std::thread::sleep;
//!
//! for i in (0..).progress() {
//!     if i == 100 {
//!         break;
//!     }
//!     sleep(Duration::from_millis(50));
//! }
//! ```
#![doc=include_str!("../images/indeterminate.html")]
//!
//! ## Styling
//!
//! It is currently not possible to style bars in any way.
//!
//! ## Alternative crates
//!
//! * [Indicatif](https://docs.rs/indicatif/latest/indicatif/) - A crate which supports progress bars and spinners and lots of styling.
//!    However it is less ergonomic, especially when working with multiple progress bars. It also interacts poorly with simultaneous printing to stdout.

use lazy_static::lazy_static;
use std::collections::hash_map::DefaultHasher;
use std::fmt::Write;
use std::hash::{Hash, Hasher};
use std::ops::Range;
use std::thread;
use std::time::{Duration, Instant};
mod progressbar;
mod splitting;
pub use progressbar::{ProgressBar, ProgressBarIterable, ProgressBarIterator};
pub use splitting::*;

use std::{
    io::stdout,
    sync::{Arc, Mutex},
};

const BAR_FILLED: char = '█';
const BAR_EMPTY: char = ' ';
const BAR_ABANDONED: char = 'X';
const BAR_PARTIALLY_FILLED: [char; 9] = [BAR_EMPTY, '▏', '▎', '▍', '▌', '▋', '▊', '▉', '█'];
const BAR_LEFT_BORDER: char = '▕';
const BAR_RIGHT_BORDER: char = '▏';
// const BAR_UNKNOWN: char = '░';
// const BAR_UNKNOWN_ANIM: [char; 4] = ['░', '▒', '▓', '█'];

lazy_static! {
    pub(crate) static ref MANAGER: Arc<Mutex<ProgressBarManager>> =
        Arc::new(Mutex::new(ProgressBarManager {
            bars: vec![],
            thread_started: false,
            interactive_output: atty::is(atty::Stream::Stdout),
            reference_time: Instant::now(),
        }));
}

#[derive(PartialEq, Eq, Clone, Copy)]
enum LifecycleState {
    InProgress,
    Completed,
    Abandoned,
}

impl Default for LifecycleState {
    fn default() -> Self {
        Self::InProgress
    }
}

#[derive(Clone)]
struct NestedBars {
    bars: Vec<Arc<Mutex<ProgressBarState>>>,
    meta: NestedMeta,
}

#[derive(Clone)]
enum NestedMeta {
    Weighted(Vec<f64>),
    Sized(Vec<f64>),
    Summed,
}

#[derive(Clone, Default)]
struct ProgressBarState {
    pub length: Option<usize>,
    pub position: usize,
    pub message: Option<String>,
    pub nested: Option<NestedBars>,
    pub lifecycle: LifecycleState,
}

impl ProgressBarState {
    fn hash_state(&self, state: &mut impl Hasher) {
        self.length.hash(state);
        self.position.hash(state);
        self.message.hash(state);
        if let Some(nested) = &self.nested {
            for b in &nested.bars {
                b.lock().unwrap().hash_state(state);
            }
        }
    }

    fn progress_count(&self) -> (f64, f64, f64, f64, Option<f64>) {
        if let Some(nested) = &self.nested {
            let mut total_lower_len = 0.0;
            let mut total_upper_len = Some(0.0);
            let mut total_progress = 0.0;
            let mut total_abandoned = 0.0;
            let mut total_in_progress = 0.0;
            match &nested.meta {
                NestedMeta::Sized(weights) | NestedMeta::Weighted(weights) => {
                    for (w, bar) in weights.iter().zip(&nested.bars) {
                        let (mut progress, mut in_progress, abandoned, lower_len, upper_len) =
                            bar.lock().unwrap().progress_count();

                        total_lower_len += w;
                        total_upper_len = total_upper_len.map(|x| x + w);

                        if upper_len.is_none() {
                            if lower_len == 0.0 {
                                // If the child bar has no known upper bound on its length we normally cannot say anything other than that its in progress.
                                // However, if the bar has made no actual progress then this is a well defined state.
                                // This is important because when splitting bars and working on tasks sequentially, often the bars that come
                                // later have no well defined length before the program actually starts working on them.
                                progress = 0.0;
                                in_progress = 0.0;
                            } else {
                                // If we don't know the upper bound on the length of the child bar, then we can't say anything other than that
                                // things are in progress, but we don't actually know the percentage progress at all.
                                progress = 0.0;
                                in_progress = 1.0 - abandoned;
                            }
                        }
                        total_progress += (progress as f64) * w;
                        total_abandoned += (abandoned as f64) * w;
                        total_in_progress += (in_progress as f64) * w;
                    }

                    match nested.meta {
                        NestedMeta::Weighted(_) => {
                            // A weighted split is based on fractions. So a natural default is that the whole bar has a size of 1
                            total_lower_len = total_lower_len.max(1.0);
                            total_upper_len = total_upper_len.map(|x| x.max(1.0));
                        }
                        NestedMeta::Sized(_) => {
                            // If the user has manually specified a size for the parent bar then we use that
                            if let Some(length) = self.length {
                                total_lower_len = total_lower_len.max(length as f64);
                                total_upper_len = total_upper_len.map(|x| x.max(length as f64));
                            }
                        }
                        _ => {}
                    }
                }
                NestedMeta::Summed => {
                    for bar in &nested.bars {
                        let (progress, in_progress, abandoned, lower_len, upper_len) =
                            bar.lock().unwrap().progress_count();

                        if progress + in_progress + abandoned > 0.0 && upper_len.is_none() {
                            total_upper_len = None;
                        }
                        total_lower_len += lower_len;
                        let upper_len = upper_len.unwrap_or(lower_len);
                        total_upper_len = total_upper_len.map(|x| x + upper_len);

                        total_progress += progress * lower_len as f64;
                        total_abandoned += abandoned * lower_len as f64;
                        total_in_progress += in_progress * lower_len as f64;
                    }

                    if let Some(length) = self.length {
                        // If the user has manually specified a size for the parent bar then we use that
                        total_lower_len = total_lower_len.max(length as f64);
                        if length as f64 >= total_lower_len {
                            total_upper_len = total_upper_len.or(Some(length as f64));
                        }
                        total_upper_len = total_upper_len.map(|x| x.max(length as f64));
                    }
                }
            }

            if total_lower_len > 0.0 {
                total_progress /= total_lower_len;
                total_abandoned /= total_lower_len;
                total_in_progress /= total_lower_len;
            }

            (
                total_progress,
                total_in_progress,
                total_abandoned,
                total_lower_len,
                total_upper_len,
            )
        } else {
            // This is a leaf progress bar
            if let Some(length) = self.length {
                if length > 0 {
                    let clamped_pos = self.position.min(length);
                    let abandoned_length = if self.lifecycle == LifecycleState::Abandoned {
                        length - clamped_pos
                    } else {
                        0
                    };
                    (
                        clamped_pos as f64 / length as f64,
                        0.0,
                        abandoned_length as f64 / length as f64,
                        length as f64,
                        Some(length as f64),
                    )
                } else {
                    (
                        if self.lifecycle == LifecycleState::Completed {
                            1.0
                        } else {
                            0.0
                        },
                        0.0,
                        if self.lifecycle == LifecycleState::Abandoned {
                            1.0
                        } else {
                            0.0
                        },
                        0.0,
                        Some(0.0),
                    )
                }
            } else {
                // The bar has an unknown length
                if self.lifecycle != LifecycleState::InProgress {
                    // If it's finished the final position becomes the length
                    if self.lifecycle == LifecycleState::Abandoned && self.position == 0 {
                        // If the bar was abandoned without any progress being made, then mark 100% of it as abandoned
                        (0.0, 0.0, 1.0, 0.0, Some(0.0))
                    } else {
                        (
                            1.0,
                            0.0,
                            0.0,
                            self.position as f64,
                            Some(self.position as f64),
                        )
                    }
                } else {
                    (1.0, 0.0, 0.0, self.position as f64, None)
                }
            }
        }
    }

    fn progress(&self) -> Option<f64> {
        let (progress, _in_progress, _abandoned, lower_len, upper_len) = self.progress_count();
        if let Some(upper_len) = upper_len {
            if upper_len > 0.0 {
                Some((progress * lower_len / (upper_len as f64)).clamp(0.0, 1.0))
            } else {
                Some(0.0)
            }
        } else {
            None
        }
    }

    fn visit_completed(&self, visitor: &mut impl FnMut(bool, &ProgressBarState)) -> bool {
        if let Some(nested) = &self.nested {
            let mut completed = true;
            for b in &nested.bars {
                completed &= b.lock().unwrap().visit_completed(visitor);
            }
            visitor(completed, self);
            completed
        } else {
            let completed = self.length.map(|l| self.position >= l).unwrap_or(false)
                || self.lifecycle != LifecycleState::InProgress;
            visitor(completed, self);
            completed
        }
    }

    /// Number of external references to the children of this bar.
    fn nested_strong_count(&self) -> usize {
        if let Some(nested) = &self.nested {
            nested
                .bars
                .iter()
                .map(|b| (Arc::strong_count(b) - 1) + b.lock().unwrap().nested_strong_count())
                .sum::<usize>()
        } else {
            0
        }
    }

    fn message(&self) -> Option<String> {
        // Message of first non-completed bar
        // or last completed bar
        let mut msg = None;
        let all_completed = self.visit_completed(&mut |completed, bar| {
            if !completed && msg.is_none() {
                msg = bar.message.clone();
            }
        });
        if all_completed {
            // Last completed bar
            self.visit_completed(&mut |_, bar| {
                if bar.message.is_some() {
                    // TODO: Kinda suboptimal
                    msg = bar.message.clone();
                }
            });
        }

        msg
    }

    fn render_indeterminate_bar(out: &mut String, steps: Range<usize>, reference_time: &Instant) {
        let t = reference_time.elapsed().as_secs_f64();
        for i in steps {
            const BRIGHTNESS_STEPS: usize = 24;
            let anim_index = ((((2.0 * t + (i as f64) * 0.7).sin() * 0.5 + 0.5)
                * BRIGHTNESS_STEPS as f64)
                .floor() as usize)
                .clamp(0, BRIGHTNESS_STEPS - 1);

            // SAFETY: Writes to strings cannot fail
            write!(out, "\u{001b}[38;5;{}m{}", 232 + anim_index, BAR_FILLED).unwrap();
        }
        out.push_str("\u{001b}[0m");
    }

    fn render(
        &self,
        out: &mut String,
        color: bool,
        reference_time: &Instant,
        is_animating: &mut bool,
    ) -> std::fmt::Result {
        let bar_width = 20;

        let (progress_value, in_progress_value, abandoned_value, length_lower, length_upper) =
            self.progress_count();

        debug_assert!(progress_value <= 1.0);
        debug_assert!(in_progress_value <= 1.0);
        debug_assert!(abandoned_value <= 1.0);
        debug_assert!(progress_value + in_progress_value + abandoned_value <= 1.0001);

        if let Some(length_upper) = length_upper {
            debug_assert!(length_lower <= length_upper);

            let bounds_multiplier = if length_upper > 0.0 {
                length_lower / length_upper
            } else {
                0.0
            };

            let filled_pos = progress_value * bounds_multiplier * bar_width as f64;
            let mut filled_index = filled_pos.floor() as usize;
            let mut in_progress_index =
                ((progress_value + in_progress_value) * bounds_multiplier * bar_width as f64)
                    .floor() as usize;
            let abandoned_index =
                ((1.0 - abandoned_value * bounds_multiplier) * bar_width as f64).floor() as usize;

            out.push(BAR_LEFT_BORDER);
            for _ in 0..filled_index {
                out.push(BAR_FILLED);
            }
            if filled_index < abandoned_index {
                let partially_filled_step = (filled_pos.fract() * 8.0).floor() as usize;
                if partially_filled_step > 0 {
                    filled_index += 1;
                    in_progress_index = in_progress_index.max(filled_index);
                    out.push(BAR_PARTIALLY_FILLED[partially_filled_step]);
                }
            }

            let indeterminate_range = filled_index..in_progress_index;
            *is_animating |= !indeterminate_range.is_empty();
            Self::render_indeterminate_bar(out, indeterminate_range, reference_time);

            for _ in in_progress_index..abandoned_index {
                out.push(BAR_EMPTY);
            }
            if abandoned_index < bar_width {
                if color {
                    out.push_str("\u{001b}[31m");
                }
                for _ in abandoned_index..bar_width {
                    out.push(BAR_ABANDONED);
                }
                if color {
                    out.push_str("\u{001b}[0m");
                }
            }
            out.push(BAR_RIGHT_BORDER);
        } else {
            *is_animating = true;
            out.push(BAR_LEFT_BORDER);
            Self::render_indeterminate_bar(out, 0..bar_width, reference_time);
            out.push(BAR_RIGHT_BORDER);
        }

        // Check if it's a weighted nesting. Those we always display as percentages.
        if !matches!(
            self.nested,
            Some(NestedBars {
                meta: NestedMeta::Weighted(_),
                ..
            })
        ) {
            write!(out, " {}/", (progress_value * length_lower).floor())?;
            if let Some(length_upper) = length_upper {
                write!(out, "{}", length_upper)?;
            } else {
                write!(out, "?")?;
            }
        } else if let Some(p) = self.progress() {
            write!(out, " {}%", (p * 100.0).floor() as usize)?;
        } else {
            write!(out, " ?%")?;
        }

        if let Some(msg) = self.message() {
            write!(out, " {}", msg)?;
        }

        Ok(())
    }
}

struct ProgressBarManager {
    /// All currently visible bars
    pub bars: Vec<Arc<Mutex<ProgressBarState>>>,
    /// True if the [`manager_thread`] is running
    pub thread_started: bool,
    /// True if the output is a tty (terminal)
    interactive_output: bool,
    /// An arbitrary fixed reference time
    reference_time: Instant,
}

impl ProgressBarManager {
    pub fn hash_state(&mut self) -> u64 {
        let mut hasher = DefaultHasher::new();
        hasher.write_usize(self.bars.len());
        for bar in &self.bars {
            let bar = bar.lock().unwrap();
            bar.hash_state(&mut hasher);
        }
        hasher.finish()
    }

    pub fn tick(&mut self, out: &mut impl std::io::Write) -> std::io::Result<bool> {
        let mut temp_output = String::new();
        let mut is_animating = false;

        let mut to_remove = 0;
        for bar in &self.bars {
            let b = bar.lock().unwrap();
            if Arc::strong_count(bar) + b.nested_strong_count() == 1 {
                // Only the manager has a reference to this bar. This means it has been dropped
                // everywhere else, and we can safely render it a final time and then forget about it.
                b.render(
                    &mut temp_output,
                    self.interactive_output,
                    &self.reference_time,
                    &mut is_animating,
                )
                .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e))?;
                temp_output.push('\n');
                to_remove += 1;
            } else {
                break;
            }
        }
        self.bars.drain(0..to_remove);

        if !self.interactive_output {
            // When we are not writing to a terminal, we only render progress bars when they are finished (or abandoned)
            write!(out, "{}", &temp_output)?;
            out.flush().unwrap();
            return Ok(is_animating);
        }

        for bar in &self.bars {
            bar.lock()
                .unwrap()
                .render(
                    &mut temp_output,
                    self.interactive_output,
                    &self.reference_time,
                    &mut is_animating,
                )
                .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e))?;
            temp_output.push('\n');
        }

        write!(out, "{}", &temp_output)?;

        if !self.bars.is_empty() {
            // Move to start of line N lines up
            // Together with the clearing below, this will make sure that if something is printed to stdout it will first
            // remove the progress bars and then print the text.
            let prev_lines = self.bars.len();
            write!(out, "\u{001b}[{}F", prev_lines)?;
            out.flush().unwrap();
            // then clear everything after the cursor to end of screen.
            // DO NOT flush after this as that would remove the progress bars.
            write!(out, "\u{001b}[0J")?;
        } else {
            out.flush().unwrap();
        }

        Ok(is_animating)
    }
}

/// Thread which runs while progress bars are visible
fn manager_thread() {
    let mut last_state = 0;
    let mut last_update = Instant::now();
    let mut is_animating = false;
    loop {
        {
            let stdout = stdout();
            let mut out = stdout.lock();

            let mut manager = MANAGER.lock().unwrap();
            if manager.bars.is_empty() {
                manager.thread_started = false;
                return;
            }

            let h = manager.hash_state();
            let update_period = if is_animating { 33 } else { 200 };
            if h != last_state || last_update.elapsed() > Duration::from_millis(update_period) {
                last_state = h;
                last_update = Instant::now();
                is_animating = manager.tick(&mut out).unwrap();
            }
        }
        thread::sleep(Duration::from_millis(20));
    }
}
