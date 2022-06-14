use advance::ProgressBarIterable;
use std::{thread::sleep, time::Duration};

pub fn main() {
    for i in (1..).progress() {
        if i == 100 {
            break;
        }
        sleep(Duration::from_millis(50));
    }
}
