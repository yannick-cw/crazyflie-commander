#[derive(Debug, Default)]
pub struct App {
    pub exit: bool,
    pub counter: i64,
}

impl App {
    pub fn exit(&mut self) {
        self.exit = true;
    }

    pub fn decrement(&mut self) {
        self.counter -= 1;
    }

    pub fn increment(&mut self) {
        self.counter += 1;
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn test_app_increment_counter() {
        let mut app = App::default();
        app.increment();
        assert_eq!(app.counter, 1);
    }

    #[test]
    fn test_app_decrement_counter() {
        let mut app = App::default();
        app.decrement();
        assert_eq!(app.counter, 0);
    }
}
