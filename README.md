# Headway - An ergonomic progress bar library

Headway is a progress bar library focused on ergonomics, just getting out of your way and letting you get back to writing all that other code.

![example](https://raw.githubusercontent.com/HalfVoxel/headway/main/images/multiple.svg)

## Installation

```toml
# In Cargo.toml
headway = "0.1"
```

## Usage

```rust
use headway::ProgressBarIterable;

for _ in (0..100).progress() {
    // Do important stuff here
}
```

Take a look at [the documentation](https://docs.rs/headway) for more examples.

## Advantages

Compared to other progress bar libraries, *headway*:

* Allows multiple progress bars to run concurrently out of the box, even from separate threads.
* Integrates with `stdout` so that printing to `stdout` does not mess up either your progress bars or your printed text.
* Allows easily splitting progress bars into smaller parts (makes it easy to break up tasks over multiple threads, or into semantically separate parts).
* Takes advantage of unicode to increment the progress bar more smoothly.
* Works properly even if you only increment it very seldom (many libraries will show stale data if the bar is not incremented often enough).

Take a look at [the documentation](https://docs.rs/headway) for more details.

## Disadvantages

If you are looking for a progress bar that can be styled in a variety of ways then other libraries may be better. *Headway* currently does not have any support for styling progress bars.

Take a look at [the documentation](https://docs.rs/headway) for some alternatives.

## Contributing

Pull requests are welcome! :)
