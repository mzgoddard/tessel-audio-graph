use std::time::Instant;
use std::sync::{Arc, Mutex};

use graph_utils::{Callback, CallbackInner, RingBuffer};

#[derive(Clone)]
pub struct GateState(Arc<Mutex<bool>>);

pub struct Gated(Callback);

#[derive(Clone)]
pub struct SwitchState(Arc<Mutex<usize>>);

pub struct Switched(Callback);

impl GateState {
    pub fn new() -> GateState {
        GateState(Arc::new(Mutex::new(false)))
    }

    pub fn get(&self) -> bool {
        match self.0.lock() {
            Ok(guard) => *guard,
            _ => false,
        }
    }

    pub fn map<T>(&self, mapfn: T) where T : Fn(&bool) -> bool {
        if let Ok(mut guard) = self.0.lock() {
            *guard = mapfn(&*guard);
        }
    }

    pub fn set(&self, state: bool) {
        self.map(|_| state);
    }

    pub fn toggle(&self) {
        self.map(|state| !state);
    }
}

impl CallbackInner for Gated {
    fn get_callback(&mut self) -> &mut Callback {
        &mut self.0
    }
}

impl Gated {
    pub fn new(state: GateState) -> Box<Gated> {
        Box::new(Gated(Callback::new(Box::new(move |input, output| {
            if !input.active {return;}
            if let Ok(gated) = state.0.lock() {
            // if !gate_cell.get() {
                if !*gated {
                    output.active = false;
                    input.clear();
                }
                else {
                    output.write_from_ring(input.len(), input);
                }
            }
        }))))
    }
}

impl SwitchState {
    pub fn new() -> SwitchState {
        SwitchState(Arc::new(Mutex::new(1)))
    }

    pub fn get(&self) -> usize {
        match self.0.lock() {
            Ok(guard) => *guard,
            _ => 0,
        }
    }

    pub fn set(&self, state: usize) {
        self.map(|_| state);
    }

    pub fn map<T>(&self, mapfn: T) where T : Fn(&usize) -> usize {
        if let Ok(mut guard) = self.0.lock() {
            *guard = mapfn(&*guard);
        }
    }
}

impl CallbackInner for Switched {
    fn get_callback(&mut self) -> &mut Callback {
        &mut self.0
    }
}

impl Switched {
    pub fn new(state: SwitchState, my_state: usize) -> Box<Switched> {
        Box::new(Switched(Callback::new(Box::new(move |input, output| {
            if !input.active {return;}
            if let Ok(gated) = state.0.lock() {
                if *gated != my_state {
                    output.active = false;
                    input.clear();
                }
                else {
                    output.write_from_ring(input.len(), input);
                }
            }
        }))))
    }
}
