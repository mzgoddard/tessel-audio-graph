use graph_utils::{Callback, CallbackInner, RingBuffer};

pub struct MonoToStereo(Callback);

impl CallbackInner for MonoToStereo {
    fn get_callback(&mut self) -> &mut Callback {
        &mut self.0
    }
}

impl MonoToStereo {
    pub fn new() -> Box<MonoToStereo> {
        Box::new(MonoToStereo(Callback::new(Box::new(move |input, output| {
            let avail = input.len();
            if avail > 0 {
                let slice = input.read_slice(avail);
                let mut out_slice = output.write_slice(avail * 2);
                let mut slice_iter = slice.iter();
                let mut i = 0;
                for (index, o) in out_slice.iter_mut().enumerate() {
                    if index % 2 == 0 {
                        i = *slice_iter.next().unwrap();
                    }
                    *o = i;
                }
            }
        }))))
    }
}
