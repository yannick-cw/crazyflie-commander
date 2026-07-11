use crossterm::event::{Event, EventStream};
use futures::future::BoxFuture;
use futures::stream::{BoxStream, FuturesUnordered, select_all};
use futures::{FutureExt, StreamExt};
use ratatui::Frame;
use tokio::select;

pub struct Cmd<Msg>(Vec<BoxFuture<'static, Msg>>);

pub type Sub<Msg> = Vec<BoxStream<'static, Msg>>;
// todo ergonomics: Cmd:: would be nicer?
impl<Msg: Send + 'static> Cmd<Msg> {
    pub fn none() -> Cmd<Msg> {
        Cmd(Vec::new())
    }

    // send to send across threads, 'static so future owns all data it brings, as it will live
    // longer than the calling scope
    pub fn new(cmd: impl Future<Output = Msg> + Send + 'static) -> Cmd<Msg> {
        Cmd(vec![Box::pin(cmd)])
    }

    pub fn pure(msg: Msg) -> Cmd<Msg> {
        Self::new(async move { msg })
    }

    pub fn batch(cmds: Vec<impl Future<Output = Msg> + Send + 'static>) -> Cmd<Msg> {
        cmds.into_iter().flat_map(Self::new).collect()
    }

    pub fn lift_msg<M, F>(self, f: F) -> Cmd<M>
    where
        F: Fn(Msg) -> M + Clone + Send + 'static,
        Msg: Send + 'static,
        M: Send + 'static,
    {
        Cmd(self
            .0
            .into_iter() // own the futures
            .map(|fut| {
                let f = f.clone(); // each future owns its own copy of f
                fut.map(move |msg| f(msg)).boxed() // Map<..> -> BoxFuture<'static, M>
            })
            .collect())
    }
}
impl<Msg> IntoIterator for Cmd<Msg> {
    type Item = BoxFuture<'static, Msg>;
    type IntoIter = std::vec::IntoIter<Self::Item>;

    fn into_iter(self) -> Self::IntoIter {
        self.0.into_iter()
    }
}
impl<Msg> FromIterator<BoxFuture<'static, Msg>> for Cmd<Msg> {
    fn from_iter<I: IntoIterator<Item = BoxFuture<'static, Msg>>>(it: I) -> Self {
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

pub async fn run<Model, Msg>(p: Program<Model, Msg>) -> color_eyre::Result<()> {
    let Program {
        init,
        update,
        view,
        subscriptions,
        lift_terminal_event: lift_event,
        exit_condition,
    } = p;

    let mut terminal = ratatui::init();
    let (mut model, init_cmd) = init();
    let mut in_flight: FuturesUnordered<_> = init_cmd.into_iter().collect();

    let mut subs = select_all(subscriptions);

    let mut event_stream = EventStream::new();

    // init draw
    terminal.draw(|frame| view(&model, frame))?;

    while !exit_condition.iter().any(|f| f(&model)) {
        let maybe_msg: Option<Msg> = select! {
            Some(new_msg) = in_flight.next() => Some(new_msg),
            Some(evt) = event_stream.next() => { match evt {
                    Ok(e) => lift_event.map(|f|f(e)).flatten(),
                    Err(_) => None,
                }
            },
            Some(sub_msg) = subs.next() => Some(sub_msg),
        };

        if let Some(new_msg) = maybe_msg {
            let (new_model, created_cmd) = update(new_msg, model);
            in_flight.extend(created_cmd);
            model = new_model;
            terminal.draw(|frame| view(&model, frame))?;
        }
    }
    ratatui::restore();
    Ok(())
}
