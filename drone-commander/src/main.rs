use ratatui::{DefaultTerminal, Frame};
use drone_control::setup_link;

#[tokio::main]
async fn main() -> color_eyre::Result<()> {
    let real_unit = setup_link().await?;

    color_eyre::install()?;
    ratatui::run(app)?;
    Ok(())
}

fn app(terminal: &mut DefaultTerminal) -> std::io::Result<()> {
    loop {
        terminal.draw(render)?;
        if crossterm::event::read()?.is_key_press() {
            break Ok(());
        }
    }
}

fn render(frame: &mut Frame) {
    frame.render_widget("hello world", frame.area());
}
