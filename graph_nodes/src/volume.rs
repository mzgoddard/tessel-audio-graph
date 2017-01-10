use graph_utils::{Callback, CallbackInner, RingBuffer};

pub struct Volume(Callback);

impl CallbackInner for Volume {
    fn get_callback(&mut self) -> &mut Callback {
        &mut self.0
    }
}

impl Volume {
    pub fn new((num, denom): (i32, i32)) -> Box<Volume> {
        Box::new(Volume(Callback::new(Box::new(move |input, output| {
            let avail = input.len();
            for (i, o) in input.read_slice(avail).iter().zip(output.write_slice(avail).iter_mut()) {
                *o = (*i as i32 * num / denom) as i16;
            }
        }))))
    }
}
