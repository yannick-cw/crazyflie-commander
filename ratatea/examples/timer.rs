use crate::Msg::TerminalEvt;
use crossterm::event::{Event, KeyCode};
use futures::{FutureExt, StreamExt};
use ratatea::{Cmd, Ratatea, Sub, run};
use ratatui::Frame;
use ratatui::layout::Alignment;
use ratatui::widgets::{Block, BorderType};
use std::time::Duration;
use tokio::time;
use tokio::time::sleep;
use tokio_stream::wrappers::IntervalStream;

#[tokio::main]
async fn main() -> color_eyre::Result<()> {
    run(Program).await
}

struct Program;

impl Ratatea for Program {
    type Model = Model;
    type Msg = Msg;

    fn init(&self) -> (Self::Model, Cmd<Self::Msg>) {
        (
            Model {
                elapsed_time: Duration::default(),
                should_exit: false,
            },
            Cmd::new(sleep(Duration::from_secs(3)).map(|_| Msg::Tick(Duration::from_secs(3)))),
        )
    }

    fn update(&self, msg: Self::Msg, model: Self::Model) -> (Self::Model, Cmd<Self::Msg>) {
        match msg {
            Msg::Tick(elapsed) => (
                Model {
                    elapsed_time: model.elapsed_time + elapsed,
                    ..model
                },
                Cmd::none(),
            ),
            TerminalEvt(Event::Key(k)) if k.code == KeyCode::Char('x') => (
                Model {
                    should_exit: true,
                    ..model
                },
                Cmd::none(),
            ),
            _ => (model, Cmd::none()),
        }
    }

    fn view(&self, model: &Self::Model, frame: &mut Frame) {
        let block = Block::bordered()
            .border_type(BorderType::Rounded)
            .title(format!(" ⬡ CRAZYFLIE · COMMANDER {:?}", model.elapsed_time))
            .title_alignment(Alignment::Center);
        frame.render_widget(block, frame.area())
    }

    fn subscriptions(&self, _model: &Model) -> Sub<Self::Msg> {
        let subscriptions: Sub<Msg> = {
            let tick = IntervalStream::new(time::interval(Duration::from_millis(1000)))
                .map(|_| Msg::Tick(Duration::from_millis(1000)));
            vec![tick.boxed()]
        };
        subscriptions
    }

    fn exit_condition(&self, model: &Self::Model) -> bool {
        model.should_exit
    }

    fn lift_terminal_event(&self, e: Event) -> Option<Self::Msg> {
        Some(TerminalEvt(e))
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
