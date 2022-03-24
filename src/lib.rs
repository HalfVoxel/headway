use lazy_static::lazy_static;
use ordered_float::NotNan;
use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};
use std::io::{Read, Write};
use std::panic::{self, catch_unwind};
use std::sync::atomic::Ordering;
use std::thread;
use std::time::Duration;

use std::{
    io::{stdout, IoSlice},
    sync::{Arc, Mutex},
};

#[derive(Clone, Default)]
struct ProgressBarState {
    pub length: usize,
    pub position: usize,
    pub message: Option<String>,
    pub nested: Vec<(ScaleMode, Arc<Mutex<ProgressBarState>>)>,
}

impl ProgressBarState {
    fn abandon(&mut self) {}

    fn hash_state(&self, state: &mut impl Hasher) {
        self.length.hash(state);
        self.position.hash(state);
        self.message.hash(state);
        for (f, b) in &self.nested {
            f.hash(state);
            b.lock().unwrap().hash_state(state);
        }
    }

    fn length(&self) -> usize {
        if self.nested.is_empty() {
            self.length
        } else {
            self.nested
                .iter()
                .map(|(_, b)| b.lock().unwrap().length())
                .sum::<usize>()
        }
    }

    fn progress_count(&self) -> (f64, usize) {
        if self.nested.is_empty() {
            if self.length > 0 {
                (self.position.min(self.length) as f64, self.length)
            } else {
                (0.0, self.length)
            }
        } else {
            let mut total_length = 0;
            let mut total_item_progress = 0.0;
            let mut total_normalized_progress = 0.0;
            for (prop, bar) in &self.nested {
                let nested_bar = bar.lock().unwrap();
                let (nested_prog, nested_len) = nested_bar.progress_count();
                match prop {
                    ScaleMode::PropagateLength => {
                        total_item_progress += nested_prog;
                        total_length += nested_len;
                    }
                    ScaleMode::FractionOfParent(prop) => {
                        if nested_len > 0 {
                            total_normalized_progress +=
                                prop.into_inner() * nested_prog / (nested_len as f64);
                        }
                        total_length += nested_len;
                    }
                    ScaleMode::ItemsOfParent(cnt) => {
                        if nested_len > 0 {
                            total_item_progress +=
                                (*cnt as f64) * nested_prog / (nested_len as f64);
                        }
                        total_length += cnt;
                    }
                }
            }
            if self.length > 0 {
                // TODO: self.length should be an option
                total_length = self.length;
            }
            total_item_progress += total_normalized_progress * (total_length as f64);
            total_item_progress = total_item_progress.min(total_length as f64);
            (total_item_progress, total_length)
        }
    }

    fn progress(&self) -> f64 {
        let (i, l) = self.progress_count();
        if l > 0 {
            (i / (l as f64)).clamp(0.0, 1.0)
        } else {
            0.0
        }
    }

    fn visit_completed(&self, visitor: &mut impl FnMut(bool, &ProgressBarState)) -> bool {
        if self.nested.is_empty() {
            let completed = self.position >= self.length;
            visitor(completed, self);
            completed
        } else {
            let mut completed = true;
            for (_, b) in &self.nested {
                completed &= b.lock().unwrap().visit_completed(visitor);
            }
            completed
        }
    }

    fn nested_strong_count(&self) -> usize {
        self.nested
            .iter()
            .map(|(_, b)| Arc::strong_count(b) + b.lock().unwrap().nested_strong_count())
            .sum::<usize>()
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

    fn render(&self, out: &mut impl Write) -> std::io::Result<()> {
        let bar_width = 20;
        let ProgressBarState {
            length, position, ..
        } = self;

        let p = self.progress();
        let mut output: String = String::new();
        let filled_steps = (p * bar_width as f64).floor() as usize;
        for i in 0..bar_width {
            if i < filled_steps {
                output.push(BAR_FILLED);
            } else {
                output.push(BAR_EMPTY);
            }
        }
        if self.nested.is_empty() {
            write!(out, "{} {}/{}", output, *position, *length)?;
        } else {
            write!(out, "{} {}%", output, (p * 100.0).floor() as usize)?;
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
    }));
}

//

struct ProgressBarManager {
    pub bars: Vec<Arc<Mutex<ProgressBarState>>>,
    pub thread_started: bool,
    stdout_buff: Option<gag::Hold>,
    stderr_buff: Option<gag::Hold>,
    // prev_lines: usize;
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
                b.render(&mut temp_output)?;
                write!(temp_output, "\n")?;
                to_remove += 1;
            } else {
                break;
            }
        }
        self.bars.drain(0..to_remove);
        for bar in &self.bars {
            bar.lock().unwrap().render(&mut temp_output)?;
            write!(temp_output, "\n")?;
        }

        out.write_all(&temp_output)?;
        // Flush stdout and stderr which have been buffered since the last tick
        self.stdout_buff = None;
        self.stderr_buff = None;
        out.flush().unwrap();

        if self.bars.len() > 0 {
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
    println!("Starting thread");
    loop {
        {
            let stdout = stdout();
            let mut out = stdout.lock();

            let mut manager = MANAGER.lock().unwrap();
            if manager.bars.len() == 0 {
                manager.thread_started = false;
                println!("Exiting thread");
                return;
            }

            // let h = manager.hash_state();
            manager.tick(&mut out).unwrap();
        }
        thread::sleep(Duration::from_millis(20));
    }
}

#[derive(Clone)]
pub struct ProgressBar {
    state: Option<Arc<Mutex<ProgressBarState>>>,
}

impl Drop for ProgressBar {
    fn drop(&mut self) {
        self.finish();
        // if let Some(state) = self.state {
        //     if Arc::strong_count(&state) <= 2 {
        //         // Only the manager and this instance has a reference
        //         state.lock().unwrap().abandon();
        //     }
        // }
    }
}

const BAR_FILLED: char = '█';
const BAR_EMPTY: char = '░';

#[derive(Clone, Hash)]
enum ScaleMode {
    PropagateLength,
    FractionOfParent(NotNan<f64>),
    ItemsOfParent(usize),
}

impl ProgressBar {
    pub fn new() -> Self {
        let mut manager = MANAGER.lock().unwrap();
        let state = Arc::new(Mutex::new(ProgressBarState::default()));
        manager.bars.push(state.clone());
        if !manager.thread_started {
            manager.thread_started = true;
            thread::spawn(manager_thread);
        }
        Self { state: Some(state) }
    }

    pub fn hidden() -> Self {
        let state = Arc::new(Mutex::new(ProgressBarState::default()));
        Self { state: Some(state) }
    }

    pub fn nest(&self) -> ProgressBar {
        if let Some(state) = &self.state {
            let s = Arc::new(Mutex::new(ProgressBarState::default()));
            state
                .lock()
                .unwrap()
                .nested
                .push((ScaleMode::PropagateLength, s.clone()));
            Self { state: Some(s) }
        } else {
            panic!("You cannot nest a finished progress bar");
        }
    }

    pub fn nest_item(&self) -> ProgressBar {
        if let Some(state) = &self.state {
            let s = Arc::new(Mutex::new(ProgressBarState::default()));
            state
                .lock()
                .unwrap()
                .nested
                .push((ScaleMode::ItemsOfParent(1), s.clone()));
            Self { state: Some(s) }
        } else {
            panic!("You cannot nest a finished progress bar");
        }
    }

    pub fn nest_weighted(&self, fraction_of_total: f64) -> ProgressBar {
        if let Some(state) = &self.state {
            let s = Arc::new(Mutex::new(ProgressBarState::default()));
            state.lock().unwrap().nested.push((
                ScaleMode::FractionOfParent(
                    NotNan::new(fraction_of_total).expect("fraction_of_total was NaN"),
                ),
                s.clone(),
            ));
            Self { state: Some(s) }
        } else {
            panic!("You cannot nest a finished progress bar");
        }
    }

    pub fn set_length(&self, len: usize) {
        if let Some(state) = &self.state {
            let mut state = state.lock().unwrap();
            if !state.nested.is_empty() {
                panic!("You cannot set the length of a progress bar which has nested progress bars. Set the length of the nested bars instead.");
            }
            state.length = len;
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
        } else {
            if let Some(state) = &self.state {
                state.lock().unwrap().message = Some(m);
            }
        }
    }

    pub fn inc(&self) {
        if let Some(state) = &self.state {
            let mut state = state.lock().unwrap();
            if !state.nested.is_empty() {
                panic!("You cannot increment a progress bar which has nested progress bars. Increment the nested bars instead.");
            }
            state.position += 1;
        }
    }

    pub fn tick() {}

    pub fn finish_with_message(&mut self, message: impl Into<String>) {
        self.set_message(message);
        self.finish();
    }

    pub fn finish(&mut self) {
        self.state = None;
        MANAGER
            .lock()
            .unwrap()
            .tick(&mut std::io::stdout().lock())
            .unwrap();
    }

    pub fn wrap<T, It: ExactSizeIterator<Item = T>>(self, it: It) -> ProgressBarIterator<T, It> {
        self.set_length(it.len());
        ProgressBarIterator {
            progress: self,
            inner: it,
        }
    }
}

pub struct ProgressBarIterator<T, It: ExactSizeIterator<Item = T>> {
    progress: ProgressBar,
    inner: It,
}

impl<T, It: ExactSizeIterator<Item = T>> Iterator for ProgressBarIterator<T, It> {
    type Item = T;

    fn next(&mut self) -> Option<Self::Item> {
        let r = self.inner.next();
        if r.is_none() {
            self.progress.finish();
        } else {
            self.progress.inc();
        }
        r
    }
}

impl<T, It: ExactSizeIterator<Item = T>> ExactSizeIterator for ProgressBarIterator<T, It> {
    fn len(&self) -> usize {
        self.inner.len()
    }
}
