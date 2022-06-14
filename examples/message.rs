use advance::ProgressBar;
use std::{thread::sleep, time::Duration};

pub fn main() {
    let p = ProgressBar::new().with_message("Calibrating flux capacitors");
    for _ in p.wrap(0..100) {
        sleep(Duration::from_millis(20));
    }
}
