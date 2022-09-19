use std::time::Duration;
use tokio::time::sleep;

use headway::ProgressBar;

async fn complete_in_one_second(p: ProgressBar) {
    for _ in p.wrap(0..100) {
        sleep(Duration::from_millis(10)).await;
    }
}

#[allow(unused_variables)]
#[tokio::main]
async fn main() {
    // Split the bar into bars taking up a fixed fraction of the parent
    let mut p = ProgressBar::new().split_weighted();
    let first_quarter = p.take(0.25);
    let last_three_quarters = p.take(0.75);
    complete_in_one_second(first_quarter).await;
    complete_in_one_second(last_three_quarters).await;

    // Split the bar into fixed size nested bars
    let p = ProgressBar::new();
    p.set_length(50);
    let mut p = p.split_sized();
    let first_10 = p.take(10);
    let another_30 = p.take(30);
    let last_10 = p.remaining();

    complete_in_one_second(first_10).await;
    complete_in_one_second(another_30).await;
    complete_in_one_second(last_10).await;

    // Split the bar and display it by summing the progress from each child.
    let p = ProgressBar::new().split_summed();
    let first = p.take();
    let second = p.take();

    // Complete the bars concurrently
    let handle1 = complete_in_one_second(first);
    let handle2 = complete_in_one_second(second);
    tokio::join!(handle1, handle2);

    // Split into several bars, each representing one item of the iterator
    let items = &["a", "b", "c", "d"];
    for (nested_bar, letter) in ProgressBar::new().split_each(items.iter()) {
        complete_in_one_second(nested_bar.with_message(format!("Processing {}", letter))).await;
    }
}
