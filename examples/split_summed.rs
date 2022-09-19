use headway::ProgressBar;
use std::{thread, thread::sleep, time::Duration};

#[tokio::main]
async fn main() {
    let p = ProgressBar::new().split_summed();
    let tasks = (0..4)
        .map(|_| {
            let child_bar = p.take();
            thread::spawn(move || {
                for _ in child_bar.wrap(0..100) {
                    sleep(Duration::from_millis(20));
                }
            })
        })
        .collect::<Vec<_>>();
    for task in tasks {
        task.join().unwrap()
    }
}
