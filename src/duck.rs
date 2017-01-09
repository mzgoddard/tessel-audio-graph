use std::rc::Rc;
use std::cell::Cell;
use std::time::Instant;

use graph_utils::{Callback, CallbackInner, RingBuffer};

#[derive(Clone)]
pub struct DuckState(Rc<Cell<bool>>);

pub struct Duck(Callback);

pub struct Ducked(Callback);

impl DuckState {
    pub fn new() -> DuckState {
        DuckState(Rc::new(Cell::new(false)))
    }
}

impl CallbackInner for Duck {
    fn get_callback(&mut self) -> &mut Callback {
        &mut self.0
    }
}

impl Duck {
    pub fn new(peak: i16, state: DuckState) -> Box<Duck> {
        let mut active = false;
        let mut last_peak = Instant::now();
        let mut samples = 0;
        Box::new(Duck(Callback::new(Box::new(move |input, output| {
            let avail = input.len();
            let slice = input.read_slice(avail);
            samples += slice.len() / 2;
            for i in slice.iter() {
                if *i > peak {
                    active = true;
                    state.0.set(true);
                    // last_peak = Instant::now();
                    samples = 0;
                }
            }
            if !active {
                for o in output.write_slice(avail).iter_mut() {
                    *o = 0;
                }
            }
            else {
                output.write_from_read_slice(slice.len(), &slice);
                if active && samples > 48000 {
                    active = false;
                    state.0.set(false);
                }
            }
        }))))
    }
}

impl CallbackInner for Ducked {
    fn get_callback(&mut self) -> &mut Callback {
        &mut self.0
    }
}

impl Ducked {
    pub fn new(states: Vec<DuckState>, volume: (i32, i32)) -> Box<Ducked> {
        Box::new(Ducked(Callback::new(Box::new(move |input, output| {
            if !input.active {return;}
            let avail = input.len();
            let slice = input.read_slice(avail);
            if states.iter().any(|state| state.0.get()) {
                for (i, o) in slice.iter().zip(output.write_slice(avail).iter_mut()) {
                    *o = (*i as i32 * volume.0 / volume.1) as i16;
                }
            }
            else {
                output.write_from_read_slice(avail, &slice);
            }
        }))))
    }
}
