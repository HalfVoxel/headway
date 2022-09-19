use headway::ProgressBarIterable;
use std::{thread::sleep, time::Duration};

pub fn main() {
    for _ in (0..100).progress() {
        sleep(Duration::from_millis(50));
    }
}
