use advance::ProgressBar;
use std::{thread::sleep, time::Duration};

pub fn main() {
    let mut p = ProgressBar::new().split_sized();
    // Create the bars up front so that the bar knows how many items
    // there are in total.
    let first = p.take(5).with_message("First");
    let second = p.take(20).with_message("Second");

    for _ in first.wrap(0..5) {
        sleep(Duration::from_millis(300));
    }

    // Here we only loop over 5 items, but we make the child bar represent
    // 20 items in the parent bar.
    for _ in second.wrap(0..5) {
        sleep(Duration::from_millis(300));
    }
}
