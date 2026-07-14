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

pub struct Cmd<Msg>(Vec<LocalBoxFuture<'static, Msg>>);

pub type Sub<Msg> = Vec<LocalBoxStream<'static, Msg>>;
impl<Msg: 'static> Cmd<Msg> {
    pub fn none() -> Cmd<Msg> {
        Cmd(Vec::new())
    }

    // 'static so future owns all data it brings, as it will live
    // longer than the calling scope
    pub fn new<A: 'static>(
        cmd: impl Future<Output = A> + 'static,
        to_msg: fn(A) -> Msg,
    ) -> Cmd<Msg> {
        let m = cmd.map(to_msg);
        Cmd(vec![Box::pin(m)])
    }

    pub fn pure(msg: Msg) -> Cmd<Msg> {
        Self::new(async move { msg }, |a| a)
    }

    pub fn batch(cmds: Vec<impl Future<Output = Msg> + 'static>) -> Cmd<Msg> {
        cmds.into_iter().flat_map(|o| Self::new(o, |a| a)).collect()
    }

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

pub struct Program<Model, Msg> {
    pub init: fn() -> (Model, Cmd<Msg>),
    pub update: fn(Msg, Model) -> (Model, Cmd<Msg>),
    pub view: fn(&Model, &mut Frame),
    pub subscriptions: Sub<Msg>, // fn(&Model) -> for now fixed
    pub lift_terminal_event: Option<fn(e: Event) -> Option<Msg>>,
    pub exit_condition: Option<fn(&Model) -> bool>,
}

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

pub async fn run<P: Ratatea>(p: P) -> color_eyre::Result<()> {
    let mut terminal = ratatui::init();
    if terminal::supports_keyboard_enhancement()? {
        execute!(
            stdout(),
            PushKeyboardEnhancementFlags(
                KeyboardEnhancementFlags::REPORT_EVENT_TYPES
                    | KeyboardEnhancementFlags::DISAMBIGUATE_ESCAPE_CODES,
            )
        )?
    };
    // } ratatui::restore(), execute!(stdout(), PopKeyboardEnhancementFlags) — but on
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

    if terminal::supports_keyboard_enhancement()? {
        execute!(stdout(), PopKeyboardEnhancementFlags)?
    };
    ratatui::restore();
    Ok(())
}
