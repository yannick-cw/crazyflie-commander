use crate::tui::Tui;
use crossterm::event::{Event, EventStream};
use futures::StreamExt;
use futures::future::BoxFuture;
use futures::stream::{BoxStream, FuturesUnordered, select_all};
use ratatui::backend::CrosstermBackend;
use ratatui::{Frame, Terminal};
use std::io::stderr;
use tokio::select;

pub mod tui;

pub type Cmd<Msg> = Vec<BoxFuture<'static, Msg>>;

pub type Sub<Msg> = Vec<BoxStream<'static, Msg>>;
pub struct Command;
impl Command {
    pub fn none<Msg>() -> Cmd<Msg> {
        Vec::new()
    }

    // send to send across threads, 'static so future owns all data it brings, as it will live
    // longer than the calling scope
    pub fn new<Msg>(cmd: impl Future<Output = Msg> + Send + 'static) -> Cmd<Msg> {
        vec![Box::pin(cmd)]
    }

    pub fn batch<Msg>(cmds: Vec<impl Future<Output = Msg> + Send + 'static>) -> Cmd<Msg> {
        cmds.into_iter().flat_map(Self::new).collect()
    }
}

pub struct Program<Model, Msg> {
    pub init: fn() -> (Model, Cmd<Msg>),
    pub update: fn(Msg, Model) -> (Model, Cmd<Msg>),
    pub view: fn(&Model, &mut Frame),
    pub subscriptions: Sub<Msg>, // fn(&Model) -> for now fixed
    pub lift_terminal_event: Option<fn(e: Event) -> Msg>,
    pub exit_condition: Option<fn(&Model) -> bool>,
}

pub async fn run<Model, Msg>(p: Program<Model, Msg>) -> color_eyre::Result<()> {
    let Program {
        init,
        update,
        view,
        subscriptions,
        lift_terminal_event: lift_event,
        exit_condition,
    } = p;

    let backend = CrosstermBackend::new(stderr());
    let terminal = Terminal::new(backend)?;
    let mut tui = Tui::new(terminal);
    tui.enter()?;
    let (mut model, init_cmd) = init();
    let mut in_flight: FuturesUnordered<_> = init_cmd.into_iter().collect();

    let mut subs = select_all(subscriptions);

    let mut event_stream = EventStream::new();

    // init draw
    tui.draw(|frame| view(&model, frame))?;

    while !exit_condition.iter().any(|f| f(&model)) {
        let maybe_msg: Option<Msg> = select! {
            Some(new_msg) = in_flight.next() => Some(new_msg),
            Some(evt) = event_stream.next() => { match evt {
                    Ok(e) => lift_event.map(|f|f(e)),
                    Err(_) => None,
                }
            },
            Some(sub_msg) = subs.next() => Some(sub_msg),
        };

        if let Some(new_msg) = maybe_msg {
            let (new_model, created_cmd) = update(new_msg, model);
            in_flight.extend(created_cmd);
            model = new_model;
            tui.draw(|frame| view(&model, frame))?;
        }
    }
    // Exit the user interface.
    tui.exit()?;
    Ok(())
}
