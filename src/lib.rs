use lazy_static::lazy_static;
use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};
use std::io::Write;
use std::thread;
use std::time::{Duration, Instant};

use std::{
    io::stdout,
    sync::{Arc, Mutex},
};

#[derive(PartialEq, Eq, Clone, Copy)]
enum FinishedState {
    InProgress,
    Completed,
    Abandoned,
}

impl Default for FinishedState {
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
    pub finished: FinishedState,
    pub is_nested: bool,
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
                NestedMeta::Weighted(weights) => {
                    for (w, bar) in weights.iter().zip(&nested.bars) {
                        let (progress, mut in_progress, abandoned, lower_len, upper_len) =
                            bar.lock().unwrap().progress_count();

                        total_lower_len += w;
                        total_upper_len = total_upper_len.map(|x| x + w);

                        if upper_len.is_none() {
                            in_progress = 1.0 - progress - abandoned;
                        }
                        total_progress += (progress as f64) * w;
                        total_abandoned += (abandoned as f64) * w;
                        total_in_progress += (in_progress as f64) * w;
                    }

                    total_lower_len = total_lower_len.max(1.0);
                    total_upper_len = total_upper_len.map(|x| x.max(1.0));
                }
                NestedMeta::Sized(counts) => {
                    for (cnt, bar) in counts.iter().zip(&nested.bars) {
                        let cnt = *cnt;
                        let (progress, mut in_progress, abandoned, lower_len, upper_len) =
                            bar.lock().unwrap().progress_count();

                        total_lower_len += cnt;
                        total_upper_len = total_upper_len.map(|x| x + cnt);

                        if upper_len.is_none() {
                            in_progress = 1.0 - progress - abandoned;
                        }
                        total_progress += (progress as f64) * cnt;
                        total_abandoned += (abandoned as f64) * cnt;
                        total_in_progress += (in_progress as f64) * cnt;
                    }
                    if let Some(length) = self.length {
                        total_lower_len = total_lower_len.max(length as f64);
                        total_upper_len = total_upper_len.map(|x| x.max(length as f64));
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
            debug_assert!(total_progress <= 1.0);
            debug_assert!(total_abandoned <= 1.0);
            debug_assert!(total_in_progress <= 1.0);
            debug_assert!(total_progress + total_abandoned + total_in_progress <= 1.0001);

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
                    let abandoned_length = if self.finished == FinishedState::Abandoned {
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
                        if self.finished == FinishedState::Completed {
                            1.0
                        } else {
                            0.0
                        },
                        0.0,
                        if self.finished == FinishedState::Abandoned {
                            1.0
                        } else {
                            0.0
                        },
                        0.0,
                        Some(0.0),
                    )
                }
            } else {
                // Unknown length
                if self.finished != FinishedState::InProgress {
                    // Well, it's finished, so the final position becomes the length
                    (
                        1.0,
                        0.0,
                        0.0,
                        self.position as f64,
                        Some(self.position as f64),
                    )
                } else {
                    (1.0, 0.0, 0.0, self.position as f64, None)
                }
            }
        }
    }

    /** Returns a tuple of (items of progress, items that have been abandoned, total length if known) */
    // fn progress_count(&self) -> (f64, f64, Option<usize>) {
    //     if self.nested.is_empty() {
    //         if let Some(length) = self.length {
    //             let abandoned_length = if self.finished == FinishedState::Abandoned {
    //                 length - self.position.min(length)
    //             } else {
    //                 0
    //             };
    //             (
    //                 self.position.min(length) as f64,
    //                 abandoned_length as f64,
    //                 Some(length),
    //             )
    //         } else {
    //             // Unknown length
    //             if self.finished != FinishedState::InProgress {
    //                 // Well, it's finished, so the final position becomes the length
    //                 (self.position as f64, 0.0, Some(self.position))
    //             } else {
    //                 (self.position as f64, 0.0, None)
    //             }
    //         }
    //     } else {
    //         let mut total_length = Some(0);
    //         let mut total_item_progress = 0.0;
    //         let mut total_normalized_progress = 0.0;
    //         let mut total_item_abandoned_progress = 0.0;
    //         let mut total_normalized_abandoned_progress = 0.0;
    //         for (prop, bar) in &self.nested {
    //             let nested_bar = bar.lock().unwrap();
    //             let (nested_prog, abandoned_prog, nested_len) = nested_bar.progress_count();
    //             match prop {
    //                 ScaleMode::PropagateLength => {
    //                     total_item_progress += nested_prog;
    //                     total_item_abandoned_progress += abandoned_prog;
    //                     if let Some(nested_len) = nested_len {
    //                         total_length = total_length.map(|x| x + nested_len);
    //                     } else if nested_prog == 0.0 {
    //                         // This is fine, the nested bar just hasn't had any progress whatsoever.
    //                         // Regardless of it's length we know it occupied nothing out of its given fraction.
    //                     } else {
    //                         // If the nested bar doesn't have a length, but it has some progress, then the total progress is not well defined
    //                         total_length = None;
    //                     }
    //                 }
    //                 ScaleMode::FractionOfParent(prop) => {
    //                     if let Some(nested_len) = nested_len {
    //                         if nested_len > 0 {
    //                             total_normalized_progress +=
    //                                 prop.into_inner() * nested_prog / (nested_len as f64);
    //                             total_normalized_abandoned_progress +=
    //                                 prop.into_inner() * abandoned_prog / (nested_len as f64);
    //                         }
    //                         total_length = total_length.map(|x| x + nested_len);
    //                     } else {
    //                         // Nested bar has an unknown length
    //                         if nested_prog == 0.0 {
    //                             // This is fine, the nested bar just hasn't had any progress whatsoever.
    //                             // Regardless of it's length we know it occupied nothing out of its given fraction.
    //                         } else {
    //                             // The nested progress bar has an unknown length but we cannot propagate that in a good way
    //                             total_length = None;
    //                         }
    //                     }
    //                 }
    //                 ScaleMode::ItemsOfParent(cnt) => {
    //                     if let Some(nested_len) = nested_len {
    //                         if nested_len > 0 {
    //                             total_item_progress +=
    //                                 (*cnt as f64) * nested_prog / (nested_len as f64);
    //                             total_item_abandoned_progress +=
    //                                 (*cnt as f64) * abandoned_prog / (nested_len as f64);
    //                         }
    //                         total_length = total_length.map(|x| x + cnt);
    //                     } else {
    //                         // Nested bar has an unknown length
    //                         if nested_prog == 0.0 {
    //                             // This is fine, the nested bar just hasn't had any progress whatsoever.
    //                             // Regardless of it's length we know it occupied nothing out of the `cnt` elements it represents in the parent bar
    //                             total_length = total_length.map(|x| x + cnt);
    //                         } else {
    //                             // The nested progress bar has an unknown length and some progress, and we cannot propagate that in a good way.
    //                             // This makes the progress of the parent bar unknown.
    //                             total_length = None;
    //                         }
    //                     }
    //                 }
    //             }
    //         }

    //         if let Some(length) = self.length {
    //             total_length = Some(length);
    //         }

    //         if let Some(total_length) = total_length {
    //             total_item_progress += total_normalized_progress * (total_length as f64);
    //             total_item_progress = total_item_progress.min(total_length as f64);

    //             total_item_abandoned_progress +=
    //                 total_normalized_abandoned_progress * (total_length as f64);
    //             total_item_abandoned_progress =
    //                 total_item_abandoned_progress.min(total_length as f64);

    //             (
    //                 total_item_progress,
    //                 total_item_abandoned_progress,
    //                 Some(total_length),
    //             )
    //         } else {
    //             // Total length is unknown
    //             // This also means we cannot include normalized progress in a good way
    //             (total_item_progress, total_item_abandoned_progress, None)
    //         }
    //     }
    // }

    fn progress(&self) -> Option<f64> {
        let (progress, in_progress, abandoned, lower_len, upper_len) = self.progress_count();
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
            completed
        } else {
            let completed = self.length.map(|l| self.position >= l).unwrap_or(false)
                || self.finished != FinishedState::InProgress;
            visitor(completed, self);
            completed
        }
    }

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
        // Msg of first non-completed bar
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
                    // TODO: Kind suboptimal
                    msg = bar.message.clone();
                }
            });
        }

        msg
    }

    fn render(&self, out: &mut impl Write, color: bool) -> std::io::Result<()> {
        let bar_width = 20;
        // let ProgressBarState {
        //     length, position, ..
        // } = self;

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
            let mut filled_steps =
                (progress_value * bounds_multiplier * bar_width as f64).floor() as usize;
            let abandoned_steps =
                (abandoned_value * bounds_multiplier * bar_width as f64).floor() as usize;

            write!(out, "{}", BAR_LEFT_BORDER)?;
            for _ in 0..filled_steps {
                write!(out, "{}", BAR_FILLED)?;
            }
            if filled_steps < bar_width - abandoned_steps {
                filled_steps += 1;
                let partially_filled_step =
                    ((progress_value * bounds_multiplier * bar_width as f64).fract() * 8.0).floor()
                        as usize;
                write!(out, "{}", BAR_PARTIALLY_FILLED[partially_filled_step])?;
            }

            for _ in filled_steps..bar_width - abandoned_steps {
                write!(out, "{}", BAR_EMPTY)?;
            }
            if abandoned_steps > 0 {
                if color {
                    write!(out, "\u{001b}[31m")?;
                }
                for _ in bar_width - abandoned_steps..bar_width {
                    write!(out, "{}", BAR_ABANDONED)?;
                }
                if color {
                    write!(out, "\u{001b}[0m")?;
                }
            }
            write!(out, "{}", BAR_RIGHT_BORDER)?;
        } else {
            write!(out, "{}", BAR_LEFT_BORDER)?;
            for _ in 0..bar_width {
                write!(out, "{}", BAR_UNKNOWN)?;
            }
            write!(out, "{}", BAR_RIGHT_BORDER)?;
        }
        if self.nested.is_none() {
            write!(out, " {}/", progress_value)?;
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

lazy_static! {
    static ref MANAGER: Arc<Mutex<ProgressBarManager>> = Arc::new(Mutex::new(ProgressBarManager {
        bars: vec![],
        thread_started: false,
        stdout_buff: None,
        stderr_buff: None,
        interactive_output: atty::is(atty::Stream::Stdout)
    }));
}

struct ProgressBarManager {
    pub bars: Vec<Arc<Mutex<ProgressBarState>>>,
    pub thread_started: bool,
    stdout_buff: Option<gag::Hold>,
    stderr_buff: Option<gag::Hold>,
    interactive_output: bool,
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

    pub fn tick(&mut self, out: &mut impl Write) -> std::io::Result<()> {
        let mut temp_output = vec![];

        let mut to_remove = 0;
        for bar in &self.bars {
            let b = bar.lock().unwrap();
            if Arc::strong_count(bar) + b.nested_strong_count() == 1 {
                // Only the manager has a reference to this bar
                b.render(&mut temp_output, self.interactive_output)?;
                writeln!(temp_output)?;
                to_remove += 1;
            } else {
                break;
            }
        }
        self.bars.drain(0..to_remove);

        if !self.interactive_output {
            // When we are not writing to a terminal, we only render progress bars when they are finished (or abandoned)
            out.write_all(&temp_output)?;
            out.flush().unwrap();
            return Ok(());
        }

        for bar in &self.bars {
            bar.lock()
                .unwrap()
                .render(&mut temp_output, self.interactive_output)?;
            writeln!(temp_output)?;
        }

        out.write_all(&temp_output)?;
        // Flush stdout and stderr which have been buffered since the last tick
        self.stdout_buff = None;
        self.stderr_buff = None;
        out.flush().unwrap();

        if !self.bars.is_empty() {
            // self.stdout_buff = Some(gag::Hold::stdout().unwrap());
            // self.stderr_buff = Some(gag::Hold::stderr().unwrap());

            // Move to start of line N lines up and then clear everything after the cursor to end of screen.
            // DO NOT flush after this
            let prev_lines = self.bars.len();
            write!(out, "\u{001b}[{}F\u{001b}[0J", prev_lines).unwrap();
        }

        Ok(())
    }
}

fn manager_thread() {
    let mut last_state = 0;
    let mut last_update = Instant::now();
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
            if h != last_state || last_update.elapsed() > Duration::from_millis(200) {
                last_state = h;
                last_update = Instant::now();
                manager.tick(&mut out).unwrap();
            }
        }
        thread::sleep(Duration::from_millis(20));
    }
}

pub struct ProgressBar {
    state: Option<Arc<Mutex<ProgressBarState>>>,
}

impl Drop for ProgressBar {
    fn drop(&mut self) {
        self.abandon();
    }
}

const BAR_FILLED: char = '█';
const BAR_EMPTY: char = ' ';
const BAR_UNKNOWN: char = '░';
const BAR_ABANDONED: char = 'X';
const BAR_PARTIALLY_FILLED: [char; 9] = [BAR_EMPTY, '▏', '▎', '▍', '▌', '▋', '▊', '▉', '█'];
const BAR_LEFT_BORDER: char = '▕';
const BAR_RIGHT_BORDER: char = '▏';

pub struct ProgressBarWeightedNester {
    bar: ProgressBar,
    taken_fraction: f64,
}

impl ProgressBarWeightedNester {
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

        let s = Arc::new(Mutex::new(ProgressBarState {
            is_nested: true,
            ..Default::default()
        }));
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

pub struct ProgressBarSizedNester {
    bar: ProgressBar,
    taken_count: usize,
}

impl ProgressBarSizedNester {
    pub fn take(&mut self, count: usize) -> ProgressBar {
        let s = Arc::new(Mutex::new(ProgressBarState {
            is_nested: true,
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

pub struct ProgressBarSummedNester {
    bar: ProgressBar,
}

impl ProgressBarSummedNester {
    pub fn take(&self) -> ProgressBar {
        let s = Arc::new(Mutex::new(ProgressBarState {
            is_nested: true,
            ..Default::default()
        }));
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

impl ProgressBar {
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

    pub fn hidden() -> Self {
        let state = Arc::new(Mutex::new(ProgressBarState::default()));
        Self { state: Some(state) }
    }

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

    pub fn split_each<It: Iterator>(self, it: It) -> impl Iterator<Item = (ProgressBar, It::Item)> {
        if let Some(upper_bound) = it.size_hint().1 {
            self.set_length(upper_bound);
        }
        let mut splitter = self.split_sized();
        it.map(move |v| (splitter.take(1), v))
    }

    // pub fn nest(&self) -> ProgressBar {
    //     if let Some(state) = &self.state {
    //         let s = Arc::new(Mutex::new(ProgressBarState {
    //             is_nested: true,
    //             ..Default::default()
    //         }));
    //         state
    //             .lock()
    //             .unwrap()
    //             .nested
    //             .push((ScaleMode::PropagateLength, s.clone()));
    //         Self { state: Some(s) }
    //     } else {
    //         panic!("You cannot nest a finished progress bar");
    //     }
    // }

    // pub fn nest_item(&self) -> ProgressBar {
    //     if let Some(state) = &self.state {
    //         let s = Arc::new(Mutex::new(ProgressBarState {
    //             is_nested: true,
    //             ..Default::default()
    //         }));
    //         state
    //             .lock()
    //             .unwrap()
    //             .nested
    //             .push((ScaleMode::ItemsOfParent(1), s.clone()));
    //         Self { state: Some(s) }
    //     } else {
    //         panic!("You cannot nest a finished progress bar");
    //     }
    // }

    pub fn length(&self) -> Option<usize> {
        if let Some(state) = &self.state {
            state.lock().unwrap().length
        } else {
            panic!(
                "This progress bar is finished. You can no longer retrieve information about it."
            );
        }
    }

    pub fn set_length(&self, len: usize) {
        if let Some(state) = &self.state {
            let mut state = state.lock().unwrap();
            state.length = Some(len);
        }
    }

    pub fn set_position(&self, pos: usize) {
        if let Some(state) = &self.state {
            state.lock().unwrap().position = pos;
        }
    }

    pub fn clear_message(&self) {
        if let Some(state) = &self.state {
            state.lock().unwrap().message = None;
        }
    }

    pub fn with_message(self, message: impl Into<String>) -> Self {
        self.set_message(message);
        self
    }

    pub fn set_message(&self, message: impl Into<String>) {
        let m = message.into();
        if m.is_empty() {
            self.clear_message();
        } else if let Some(state) = &self.state {
            state.lock().unwrap().message = Some(m);
        }
    }

    pub fn inc(&self) {
        if let Some(state) = &self.state {
            state.lock().unwrap().position += 1;
        }
    }

    pub fn tick() {}

    pub fn finish_with_message(&mut self, message: impl Into<String>) {
        self.set_message(message);
        self.finish();
    }

    /** Abandons the progress bar.
     *
     * The remaining part of the progress bar will be colored red to indicate it will never be completed.
     * Progress bars are automatically marked as abandoned when they are dropped.
     */
    pub fn abandon(&mut self) {
        if let Some(state) = &self.state {
            let mut state = state.lock().unwrap();
            state.finished = FinishedState::Abandoned;
        }
        self.state = None;

        let mut manager = MANAGER.lock().unwrap();
        manager.tick(&mut std::io::stdout().lock()).unwrap();
    }

    pub fn finish(&mut self) {
        if let Some(state) = &self.state {
            let mut state = state.lock().unwrap();
            if let Some(length) = state.length {
                state.position = length;
            }
            state.finished = FinishedState::Completed;
        }
        self.state = None;

        let mut manager = MANAGER.lock().unwrap();
        manager.tick(&mut std::io::stdout().lock()).unwrap();
    }

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
    fn progress(self) -> ProgressBarIterator<Self>;
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
