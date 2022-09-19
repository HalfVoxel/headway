use headway::{ProgressBar, ProgressBarIterable};
use std::time::Duration;
use tokio::time::sleep;

#[tokio::main]
async fn main() {
    let mut p = ProgressBar::new().split_weighted();

    // Take the first half of the progress bar and split it into 5 smaller parts
    let mut first_half = p.take(0.5).with_message("First part").split_sized();
    for _ in 0..5 {
        // Each of these inner tasks are 20 items each
        // They are executed concurrently
        let inner_progress = first_half.take(20);
        tokio::task::spawn(async move {
            for _ in (0..20).progress_with(inner_progress) {
                sleep(Duration::from_millis(30)).await;
            }
        });
    }

    // The second half is processed independently
    let second_half = p.take(0.5).with_message("Second part");
    for _ in (0..50).progress_with(second_half) {
        sleep(Duration::from_millis(40)).await;
    }
}
