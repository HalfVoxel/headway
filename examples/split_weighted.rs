use advance::{ProgressBar, ProgressBarIterable};
use std::{thread::sleep, time::Duration};

pub fn main() {
    let mut p = ProgressBar::new().split_weighted();
    let first_half = p.take(0.5).with_message("First part");
    let second_half = p.take(0.5).with_message("Second part");
    for _ in (0..50).progress_with(first_half) {
        sleep(Duration::from_millis(20));
    }
    for _ in (0..50).progress_with(second_half) {
        sleep(Duration::from_millis(30));
    }
}
