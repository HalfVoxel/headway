use headway::ProgressBar;
use std::{thread::sleep, time::Duration};

pub fn main() {
    let mut p = ProgressBar::new().split_sized();
    let mut flux = p.take(1).with_message("Calibrating flux capacitors");
    let mut heavy = p.take(1).with_message("Heavy work!");
    let mut important = p.take(1).with_message("More important work");

    sleep(Duration::from_millis(1000));
    flux.finish();

    sleep(Duration::from_millis(1000));
    heavy.finish();

    sleep(Duration::from_millis(1000));
    important.finish();
}
