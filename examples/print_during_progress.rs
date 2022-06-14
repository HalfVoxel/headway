use advance::ProgressBarIterable;
use std::{thread::sleep, time::Duration};

pub fn main() {
    for i in (0..100).progress() {
        if i % 10 == 0 {
            println!("{}", i);
        }
        sleep(Duration::from_millis(20));
    }
}
