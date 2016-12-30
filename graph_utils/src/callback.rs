use super::{Node, RingBuffer, copy_out_ring, BaseMix};

type CallbackFn = Box<FnMut(&mut RingBuffer, &mut RingBuffer)>;

pub struct Callback {
    base_mix: BaseMix,
    tmp_state: Option<(RingBuffer, RingBuffer, Vec<i16>)>,
    callback: CallbackFn,
}

impl Callback {
    pub fn new(callback: CallbackFn) -> Callback {
        Callback {
            base_mix: BaseMix::new(),
            tmp_state: Some((RingBuffer::new(), RingBuffer::new(), Vec::<i16>::new())),
            callback: callback,
        }
    }
}

impl Node for Callback {
    fn update(&mut self, inputs: &mut [RingBuffer], outputs: &mut [RingBuffer]) {
        let (mut in_buffer, mut out_buffer, mut sub_buffer) = self.tmp_state.take().unwrap();
        self.base_mix.mix_inputs_ring(inputs, &mut in_buffer);
        // in_buffer.write_from(in_avail, &self.base_mix.accum);
        // if outputs.len() == 1 {
        //     (self.callback)(&mut in_buffer, &mut outputs[0]);
        // }
        // else {
            // print!("{:?} ", out_buffer.buffer.as_ptr());
            out_buffer.active = in_buffer.active;
            (self.callback)(&mut in_buffer, &mut out_buffer);
            let out_avail = out_buffer.len();
            // out_buffer.read_into(out_avail, &mut sub_buffer);
            // copy_out(out_avail, &sub_buffer, outputs);
            copy_out_ring(out_avail, &mut sub_buffer, &mut out_buffer, outputs);
        // }
        self.tmp_state = Some((in_buffer, out_buffer, sub_buffer));
    }
}

#[cfg(test)]
mod test {
    use super::Callback;
    use super::super::{Node, RingBuffer};

    #[test]
    fn it_calls_back() {
        let mut a = {
            let mut buffer = Vec::<i16>::new();
            Callback::new(Box::new(move |input, output| {
                let avail = input.len();
                input.read_into(avail, &mut buffer);
                output.write_from(avail, &buffer);
            }))
        };
        let v1 = (48..96).map(|x| x as i16).collect::<Vec<i16>>();
        let mut inputs = vec!(RingBuffer::new());
        inputs[0].write_from(v1.len(), &v1);
        let mut outputs = vec!(RingBuffer::new());
        {
            let n = &mut a as &mut Node;
            n.update(&mut inputs, &mut outputs);
        }
        let mut o1 = Vec::<i16>::new();
        let avail = outputs[0].len();
        outputs[0].read_into(avail, &mut o1);
        assert_eq!(o1[0], 48);
    }
}
