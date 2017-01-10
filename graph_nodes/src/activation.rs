use std::sync::{Arc, Mutex};

#[derive(Clone)]
pub struct ActivationController {
    state: Arc<Mutex<bool>>,
}

pub enum ActivationState {
    Unavailable,
    Running,
    Activating,
}

impl ActivationState {
    pub fn available(&self) -> bool {
        match *self {
            ActivationState::Unavailable => false,
            ActivationState::Running => true,
            ActivationState::Activating => true,
        }
    }

    pub fn running(&self) -> bool {
        match *self {
            ActivationState::Unavailable => false,
            ActivationState::Running => true,
            ActivationState::Activating => false,
        }
    }

    pub fn activating(&self) -> bool {
        match *self {
            ActivationState::Unavailable => false,
            ActivationState::Running => false,
            ActivationState::Activating => true,
        }
    }
}

impl ActivationController {
    pub fn new() -> ActivationController {
        ActivationController {
            state: Arc::new(Mutex::new(false)),
        }
    }

    pub fn activate(&self) -> Option<ActivationGuard> {
        match self.state.try_lock() {
            Ok(mut guard) => {
                match *guard {
                    true => None,
                    false => {
                        *guard = true;
                        Some(ActivationGuard {state: self.state.clone()})
                    },
                }
            },
            _ => None,
        }
    }

    pub fn is_activating(&self) -> ActivationState {
        match self.state.try_lock() {
            Ok(guard) => {
                if *guard {
                    ActivationState::Activating
                }
                else {
                    ActivationState::Running
                }
            },
            _ => ActivationState::Unavailable,
        }
    }
}

pub struct ActivationGuard {
    state: Arc<Mutex<bool>>,
}

impl Drop for ActivationGuard {
    fn drop(&mut self) {
        if let Ok(mut guard) = self.state.lock() {
            *guard = false;
        }
    }
}
