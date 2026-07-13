# ratatea

An [Elm Architecture][tea] runtime for [ratatui][] TUIs, on top of tokio.

An app is a `Model`, a `Msg`, and `init` / `update` / `view` / `subscriptions`.
ratatea runs the event loop, resolving `Cmd`s and feeding back `Msg`s.

> Work in progress.

[tea]: https://guide.elm-lang.org/architecture/
[ratatui]: https://ratatui.rs

## Usage

The `Ratatea` trait is implemented for an app type and passed to `run`:

```rust
use ratatea::{run, Cmd, Ratatea, Sub};
use ratatui::{widgets::Paragraph, Frame};
use crossterm::event::{Event, KeyCode};

struct App;

impl Ratatea for App {
    type Model = i32;
    type Msg = Event;

    fn init(&self) -> (i32, Cmd<Event>) {
        (0, Cmd::none())
    }

    fn update(&self, ev: Event, count: i32) -> (i32, Cmd<Event>) {
        match ev {
            Event::Key(k) if k.code == KeyCode::Up => (count + 1, Cmd::none()),
            _ => (count, Cmd::none()),
        }
    }

    fn view(&self, count: &i32, frame: &mut Frame) {
        frame.render_widget(Paragraph::new(format!("count: {count}")), frame.area());
    }

    fn subscriptions(&self, _count: &i32) -> Sub<Event> {
        vec![]
    }

    fn lift_terminal_event(&self, e: Event) -> Option<Event> {
        Some(e)
    }
}

#[tokio::main]
async fn main() -> color_eyre::Result<()> {
    run(App).await
}
```

A complete app (ticking `Sub`, async `Cmd`, exit key) is in
[`examples/timer.rs`](examples/timer.rs):

```sh
cargo run --example timer -p ratatea
```
