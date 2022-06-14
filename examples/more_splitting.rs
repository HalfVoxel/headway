use advance::ProgressBar;

#[allow(unused_variables)]
#[tokio::main]
async fn main() {
    // Split the bar into bars taking up a fixed fraction of the parent
    let mut p = ProgressBar::new().split_weighted();
    let first_quarter = p.take(0.25);
    let last_three_quarters = p.take(0.75);

    // Split the bar into fixed size nested bars
    let p = ProgressBar::new();
    p.set_length(50);
    let mut p = p.split_sized();
    let first_10 = p.take(10);
    let another_30 = p.take(30);
    let last_10 = p.remaining();

    // Split the bar and display it by summing the progress from each child
    let p = ProgressBar::new().split_summed();
    let first = p.take();
    let second = p.take();

    // Split into several bars, each representing one item of the iterator
    let items = &["a", "b", "c", "d"];
    for (nested_bar, letter) in ProgressBar::new().split_each(items.iter()) {}
}
