use crate::model::MissionPlan;
use color_eyre::Result;
use crossterm::event::KeyEventKind;
use drone_control::Telemetry;
use futures::StreamExt;
use ratatui::crossterm::event::{self, Event as CrosstermEvent, KeyEvent};
use std::io;
use std::time::Duration;
use tokio::sync::mpsc::{UnboundedReceiver, UnboundedSender};
use tokio::sync::{mpsc, watch};
use tokio::{select, spawn, time};

#[derive(Clone, Debug)]
pub enum Message {
    /// Terminal tick.
    Tick(Telemetry),
    /// Key press.
    Key(KeyEvent),
    Quit,
    Home(NavigationMessage),
    MissionSelect(MissionSelectMessage),
    MissionExecution(),
}

#[derive(Clone, Debug)]
pub enum MissionSelectMessage {
    Nav(NavigationMessage),
    Selected(MissionPlan),
}

#[derive(Clone, PartialEq, Copy, Debug)]
pub enum NavigationMessage {
    Up,
    Down,
    Select,
}

/// Terminal event handler.
#[derive(Debug)]
pub struct EventHandler {
    receiver: UnboundedReceiver<Message>,
}

fn handle_crossterm(evt: Option<io::Result<CrosstermEvent>>, sender: &UnboundedSender<Message>) {
    if let Some(Ok(CrosstermEvent::Key(e))) = evt {
        if e.kind == KeyEventKind::Press {
            let _ = sender.send(Message::Key(e));
        }
    }
}

impl EventHandler {
    // Constructs a new instance of [`EventHandler`].
    pub fn new(tick_rate: u64, telemetry: watch::Receiver<Telemetry>) -> Self {
        let (sender, receiver) = mpsc::unbounded_channel();

        spawn(async move {
            let mut ticks = time::interval(Duration::from_millis(tick_rate));
            let mut event_stream = event::EventStream::new();

            loop {
                let delay = ticks.tick();
                let next_evt = event_stream.next();

                // either an event is fired or we tick forward after tick rate
                select! {
                    maybe_evt = next_evt => handle_crossterm(maybe_evt, &sender),
                    _ = delay          => {
                        let tele = *telemetry.borrow();
                        sender.send(Message::Tick(tele)).unwrap();
                    },
                }
            }
        });
        Self { receiver }
    }

    /// Receive the next event from the handler thread.
    ///
    /// This function will always block the current thread if
    /// there is no data available and it's possible for more data to be sent.
    pub async fn next(&mut self) -> Result<Message> {
        self.receiver
            .recv()
            .await
            .ok_or(color_eyre::eyre::eyre!("Unable to get event"))
    }
}
