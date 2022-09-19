use headway::ProgressBarIterable;
use std::{thread, thread::sleep, time::Duration};

#[tokio::main]
async fn main() {
    thread::spawn(|| {
        for i in (0..100).progress() {
            if i == 20 {
                println!("Something went wrong!");
                return;
            }
            sleep(Duration::from_millis(50));
        }
    })
    .join()
    .ok();
}
