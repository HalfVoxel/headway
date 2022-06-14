use advance::ProgressBar;
use std::{thread::sleep, time::Duration};

pub fn main() {
    let p = ProgressBar::new();
    // Split the progress bar into 10 nested bars
    for (nested_bar, i) in p.split_each(0..10) {
        // Wrap the nested bar around an iterator representing this subtask
        nested_bar.set_message(format!("Subtask {}", i));
        for _ in nested_bar.wrap(0..200) {
            sleep(Duration::from_millis(5));
        }
    }
}
