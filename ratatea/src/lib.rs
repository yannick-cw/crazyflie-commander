//! [ELM](https://guide.elm-lang.org/architecture/) architecture inspired TUI on top of
//! ratatui.
//! Users need to implement the [`Ratatea`] trait.
//!
//! # Examples
//! ```no_run
//!
//! use crossterm::event::Event;
//! use ratatea::{Cmd, Ratatea, Sub, run};
//! use ratatui::Frame;
//! use ratatui::widgets::Block;
//!
//! #[tokio::main]
//! async fn main() -> color_eyre::Result<()> {
//!     run(Program).await
//! }
//!
//! struct Program;
//!
//! impl Ratatea for Program {
//!     type Model = Model;
//!     type Msg = Msg;
//!     fn init(&self) -> (Self::Model, Cmd<Self::Msg>) {
//!         (
//!             Model {
//!                 msg: "".to_string(),
//!             },
//!             Cmd::none(),
//!         )
//!     }
//!     fn update(&self, msg: Self::Msg, model: Self::Model) -> (Self::Model, Cmd<Self::Msg>) {
//!         match msg {
//!             Msg::Hello => (model, Cmd::none()),
//!         }
//!     }
//!
//!     fn view(&self, model: &Self::Model, frame: &mut Frame) {
//!         let block = Block::bordered().title(format!("Hello? {:?}", model.msg));
//!         frame.render_widget(block, frame.area())
//!     }
//!
//!     fn subscriptions(&self, _model: &Model) -> Sub<Self::Msg> {
//!         vec![]
//!     }
//!
//!     fn exit_condition(&self, model: &Self::Model) -> bool {
//!         false
//!     }
//!
//!     fn lift_terminal_event(&self, e: Event) -> Option<Self::Msg> {
//!         None
//!     }
//! }
//!
//! struct Model {
//!     msg: String,
//! }
//!
//! enum Msg {
//!     Hello,
//! }
//! ``
use crossterm::event::{
    Event, EventStream, KeyboardEnhancementFlags, PopKeyboardEnhancementFlags,
    PushKeyboardEnhancementFlags,
};
use crossterm::{execute, terminal};
use futures::future::LocalBoxFuture;
use futures::stream::{FuturesUnordered, LocalBoxStream, select_all};
use futures::{FutureExt, StreamExt};
use ratatui::Frame;
use std::io::stdout;
use tokio::select;

/// A batch of effects to run, each yields a message fed back into [`Ratatea::update`].
///
/// Build with [`Cmd::new`], [`Cmd::pure`], [`Cmd::batch`], or [`Cmd::none`].
pub struct Cmd<Msg>(Vec<LocalBoxFuture<'static, Msg>>);

/// Ongoing message sources (streams) the runtime polls for the whole program.
///
/// Returned from [`Ratatea::subscriptions`].
pub type Sub<Msg> = Vec<LocalBoxStream<'static, Msg>>;

impl<Msg: 'static> Cmd<Msg> {
    /// A command that does nothing.
    pub fn none() -> Cmd<Msg> {
        Cmd(Vec::new())
    }

    /// Creates a new [`Cmd`] and chains the [`Msg`] to the result.
    // 'static so future owns all data it brings, as it will live
    // longer than the calling scope
    pub fn new<A: 'static>(
        cmd: impl Future<Output = A> + 'static,
        to_msg: fn(A) -> Msg,
    ) -> Cmd<Msg> {
        let m = cmd.map(to_msg);
        Cmd(vec![Box::pin(m)])
    }

    /// A command that produces `msg` immediately without doing any work.
    pub fn pure(msg: Msg) -> Cmd<Msg> {
        Self::new(async move { msg }, |a| a)
    }

    /// Combine several message producing futures into one command.
    pub fn batch(cmds: Vec<impl Future<Output = Msg> + 'static>) -> Cmd<Msg> {
        cmds.into_iter().flat_map(|o| Self::new(o, |a| a)).collect()
    }

    /// Map every message this command will produce through `f`, e.g. to wrap a child's
    /// message in the parent's message type.
    pub fn lift_msg<M, F>(self, f: F) -> Cmd<M>
    where
        F: Fn(Msg) -> M + Clone + 'static,
        Msg: 'static,
        M: 'static,
    {
        Cmd(self
            .0
            .into_iter() // own the futures
            .map(|fut| {
                let f = f.clone(); // each future owns its own copy of f
                fut.map(move |msg| f(msg)).boxed_local() // Map<..> -> BoxFuture<'static, M>
            })
            .collect())
    }
}
impl<Msg> IntoIterator for Cmd<Msg> {
    type Item = LocalBoxFuture<'static, Msg>;
    type IntoIter = std::vec::IntoIter<Self::Item>;

    fn into_iter(self) -> Self::IntoIter {
        self.0.into_iter()
    }
}
impl<Msg> FromIterator<LocalBoxFuture<'static, Msg>> for Cmd<Msg> {
    fn from_iter<I: IntoIterator<Item = LocalBoxFuture<'static, Msg>>>(it: I) -> Self {
        Cmd(it.into_iter().collect())
    }
}

/// Trait for the application to run: needs the model, message type, and how to init, update, and render.
///
/// Implement this and pass it to [`run`]. `update` returns the next model plus any [`Cmd`]s to
/// perform. [`subscriptions`](Self::subscriptions) are evaluated every loop. Terminal
/// input can be accessed through [`lift_terminal_event`](Self::lift_terminal_event).
pub trait Ratatea {
    type Model;
    type Msg;
    fn init(&self) -> (Self::Model, Cmd<Self::Msg>);
    fn update(&self, msg: Self::Msg, model: Self::Model) -> (Self::Model, Cmd<Self::Msg>);
    fn view(&self, model: &Self::Model, frame: &mut Frame);

    // for now subscriptions are only called once on start - not when the model changes
    fn subscriptions(&self, model: &Self::Model) -> Sub<Self::Msg>;

    fn exit_condition(&self, _model: &Self::Model) -> bool {
        false
    }
    fn lift_terminal_event(&self, _e: Event) -> Option<Self::Msg> {
        None
    }
}

/// Runs the application: sets up the terminal, then loops and applies each message with `update`,
/// performs the resulting [`Cmd`]s, and redraws, until [`Ratatea::exit_condition`] becomes true.
///
/// # Errors
/// Fails if terminal setup, input or drawing fails.
pub async fn run<P: Ratatea>(p: P) -> color_eyre::Result<()> {
    let mut terminal = ratatui::init();
    let supports_enhancements = terminal::supports_keyboard_enhancement()?;
    if supports_enhancements {
        execute!(
            stdout(),
            PushKeyboardEnhancementFlags(
                KeyboardEnhancementFlags::REPORT_EVENT_TYPES
                    | KeyboardEnhancementFlags::DISAMBIGUATE_ESCAPE_CODES,
            )
        )?
    };

    let (mut model, init_cmd) = p.init();
    let mut in_flight: FuturesUnordered<_> = init_cmd.into_iter().collect();

    // todo re evaluate subs and activate / deactivate (could change on model change)
    let mut subs = select_all(p.subscriptions(&model));

    let mut event_stream = EventStream::new();

    // init draw
    terminal.draw(|frame| p.view(&model, frame))?;

    while !p.exit_condition(&model) {
        let maybe_msg: Option<P::Msg> = select! {
            Some(new_msg) = in_flight.next() => Some(new_msg),
            Some(evt) = event_stream.next() => { match evt {
                    Ok(e) => p.lift_terminal_event(e),
                    Err(_) => None,
                }
            },
            Some(sub_msg) = subs.next() => Some(sub_msg),
        };

        if let Some(new_msg) = maybe_msg {
            let (new_model, created_cmd) = p.update(new_msg, model);
            in_flight.extend(created_cmd);
            model = new_model;
            terminal.draw(|frame| p.view(&model, frame))?;
        }
    }

    if supports_enhancements {
        execute!(stdout(), PopKeyboardEnhancementFlags)?
    };
    ratatui::restore();
    Ok(())
}
