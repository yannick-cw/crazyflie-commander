use crate::Msg::TerminalEvt;
use crossterm::event::{Event, KeyCode};
use futures::{FutureExt, StreamExt};
use ratatea::{Cmd, Command, Program, Sub, run};
use ratatui::Frame;
use ratatui::layout::Alignment;
use ratatui::widgets::{Block, BorderType};
use std::time::Duration;
use tokio::time;
use tokio::time::sleep;
use tokio_stream::wrappers::IntervalStream;

#[tokio::main]
async fn main() -> color_eyre::Result<()> {
    run(program()).await
}

fn program() -> Program<Model, Msg> {
    let subscriptions: Sub<Msg> = {
        let tick = IntervalStream::new(time::interval(Duration::from_millis(1000)))
            .map(|_| Msg::Tick(Duration::from_millis(1000)));
        vec![tick.boxed()]
    };

    Program {
        init,
        update,
        view,
        subscriptions,
        lift_terminal_event: Some(lift_event),
        exit_condition: Some(|m| m.should_exit),
    }
}

#[derive(Copy, Clone)]
struct Model {
    elapsed_time: Duration,
    should_exit: bool,
}

#[derive(Debug)]
enum Msg {
    Tick(Duration),
    TerminalEvt(Event),
}

fn lift_event(e: Event) -> Msg {
    TerminalEvt(e)
}

fn init() -> (Model, Cmd<Msg>) {
    (
        Model {
            elapsed_time: Duration::default(),
            should_exit: false,
        },
        Command::new(sleep(Duration::from_secs(3)).map(|_| Msg::Tick(Duration::from_secs(3)))),
    )
}

fn update(msg: Msg, model: Model) -> (Model, Cmd<Msg>) {
    match msg {
        Msg::Tick(elapsed) => (
            Model {
                elapsed_time: model.elapsed_time + elapsed,
                ..model
            },
            Command::none(),
        ),
        TerminalEvt(Event::Key(k)) if k.code == KeyCode::Char('x') => (
            Model {
                should_exit: true,
                ..model
            },
            Command::none(),
        ),
        _ => (model, Command::none()),
    }
}

fn view(model: &Model, frame: &mut Frame) {
    let block = Block::bordered()
        .border_type(BorderType::Rounded)
        .title(format!(" ⬡ CRAZYFLIE · COMMANDER {:?}", model.elapsed_time))
        .title_alignment(Alignment::Center);
    frame.render_widget(block, frame.area())
}
