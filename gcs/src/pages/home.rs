use crate::program::NavigationMessage;
use crate::program::NavigationMessage::{Down, Select, Up};
use Msg::Nav;
use crossterm::event::{KeyCode, KeyEvent};
use ratatea::Cmd;

// model -----------------------------------------
#[derive(Debug)]
pub struct Model {
    pub selected_mode: ModeSelection,
}

#[derive(Debug, PartialEq)]
pub enum ModeSelection {
    MissionSelect,
    MissionPlan,
    ManualControl,
}

impl ModeSelection {
    pub fn next(&self) -> Self {
        match self {
            ModeSelection::MissionSelect => ModeSelection::MissionPlan,
            ModeSelection::MissionPlan => ModeSelection::ManualControl,
            ModeSelection::ManualControl => ModeSelection::MissionSelect,
        }
    }
    pub fn prev(&self) -> Self {
        match self {
            ModeSelection::MissionSelect => ModeSelection::ManualControl,
            ModeSelection::MissionPlan => ModeSelection::MissionSelect,
            ModeSelection::ManualControl => ModeSelection::MissionPlan,
        }
    }
}

// msg -----------------------------------------
#[derive(Debug, Clone)]
pub enum Msg {
    Nav(NavigationMessage),
}

// update -----------------------------------------
pub fn update(model: &mut Model, msg: Msg) -> Cmd<Msg> {
    match msg {
        Nav(Up) => {
            model.selected_mode = model.selected_mode.prev();
            Cmd::none()
        }
        Nav(Down) => {
            model.selected_mode = model.selected_mode.next();
            Cmd::none()
        }
        // handled by parent - transition out
        Nav(Select) => Cmd::none(),
    }
}

pub fn map_key_evt(k: KeyEvent, _s: &Model) -> Cmd<Msg> {
    match k.code {
        KeyCode::Char('j') | KeyCode::Down if k.is_press() => Cmd::pure(Nav(Down)),
        KeyCode::Char('k') | KeyCode::Up if k.is_press() => Cmd::pure(Nav(Up)),
        KeyCode::Enter if k.is_press() => Cmd::pure(Nav(Select)),
        _ => Cmd::none(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use proptest::prelude::*;
    use proptest_state_machine::{ReferenceStateMachine, StateMachineTest, prop_state_machine};

    prop_state_machine! {
        #[test]
        fn run_nav_test(
            sequential
            // The number of transitions to be generated for each case.
            1..10
            => Model
        );
    }

    pub struct NavigationStateMachine;

    impl ReferenceStateMachine for NavigationStateMachine {
        type State = i32;
        type Transition = Msg;

        fn init_state() -> BoxedStrategy<Self::State> {
            Just(0).boxed()
        }

        fn transitions(_state: &Self::State) -> BoxedStrategy<Self::Transition> {
            prop_oneof![Just(Nav(Down)), Just(Nav(Up)), Just(Nav(Select)),].boxed()
        }

        fn apply(state: Self::State, transition: &Self::Transition) -> Self::State {
            match transition {
                Nav(Up) => state - 1,
                Nav(Down) => state + 1,
                Nav(Select) => state,
            }
        }
    }

    impl StateMachineTest for Model {
        type SystemUnderTest = Self;
        type Reference = NavigationStateMachine;

        fn init_test(
            ref_state: &<Self::Reference as ReferenceStateMachine>::State,
        ) -> Self::SystemUnderTest {
            let pos = ref_state.rem_euclid(3);
            Model {
                selected_mode: match pos {
                    0 => ModeSelection::MissionSelect,
                    1 => ModeSelection::MissionPlan,
                    2 => ModeSelection::ManualControl,
                    _ => panic!("invalid state"),
                },
            }
        }

        fn apply(
            state: Self::SystemUnderTest,
            _ref_state: &<Self::Reference as ReferenceStateMachine>::State,
            transition: <Self::Reference as ReferenceStateMachine>::Transition,
        ) -> Self::SystemUnderTest {
            let mut model = state;
            let created_cmd = update(&mut model, transition);
            assert!(created_cmd.is_empty());
            model
        }

        fn check_invariants(
            state: &Self::SystemUnderTest,
            ref_state: &<Self::Reference as ReferenceStateMachine>::State,
        ) {
            let expected_pos = match ref_state.rem_euclid(3) {
                0 => ModeSelection::MissionSelect,
                1 => ModeSelection::MissionPlan,
                2 => ModeSelection::ManualControl,
                _ => panic!("nope"),
            };
            assert_eq!(expected_pos, state.selected_mode);
        }
    }
}
