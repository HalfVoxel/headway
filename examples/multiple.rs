use advance::ProgressBarIterable;
use std::{thread, thread::sleep, time::Duration};

pub fn main() {
    let mut handles = vec![];
    for i in 0..5 {
        handles.push(thread::spawn(move || {
            for _ in (0..100).progress() {
                sleep(Duration::from_millis(20 + i * 20));
            }
        }));
    }
    for handle in handles {
        handle.join().unwrap();
    }
}
