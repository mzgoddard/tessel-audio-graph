use std::any::Any;

use super::{Node, RingBuffer, copy_out_ring, BaseMix};

type CallbackFn = Box<FnMut(&mut RingBuffer, &mut RingBuffer)>;

pub struct Callback {
    base_mix: BaseMix,
    tmp_state: Option<(RingBuffer, RingBuffer, Vec<i16>)>,
    callback: CallbackFn,
}

pub trait CallbackInner : Any {
    fn get_callback(&mut self) -> &mut Callback;
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

// impl CallbackInner for Callback {
//     fn callback(&mut self) -> &mut CallbackFn {
//         self.callback
//     }
// }

// impl<T: (Callback)> CallbackInner for T {
//     fn get_callback(&mut self) -> &mut Callback {
//         &mut self.0
//     }
// }

impl<T : CallbackInner> Node for T {
    fn update(&mut self, inputs: &mut [RingBuffer], outputs: &mut [RingBuffer]) {
        self.get_callback().update(inputs, outputs);
    }
}

impl Node for Callback {
    fn update(&mut self, inputs: &mut [RingBuffer], outputs: &mut [RingBuffer]) {
        if inputs.len() == 0 && outputs.len() == 0 {
            return;
        }
        if inputs.len() == 0 && outputs.len() == 1 {
            let (mut in_buffer, mut out_buffer, mut sub_buffer) = self.tmp_state.take().unwrap();
            let mut output = &mut outputs[0];
            output.active = in_buffer.active;
            (self.callback)(&mut in_buffer, output);
            self.tmp_state = Some((in_buffer, out_buffer, sub_buffer));
        }
        if inputs.len() == 1 && outputs.len() == 0 {
            let (mut in_buffer, mut out_buffer, mut sub_buffer) = self.tmp_state.take().unwrap();
            let mut input = &mut inputs[0];
            out_buffer.active = input.active;
            (self.callback)(input, &mut out_buffer);
            self.tmp_state = Some((in_buffer, out_buffer, sub_buffer));
        }
        else if inputs.len() == 1 && outputs.len() == 1 {
            let mut input = &mut inputs[0];
            let mut output = &mut outputs[0];
            output.active = input.active;
            (self.callback)(input, output);
        }
        else if inputs.len() == 1 {
            let (mut in_buffer, mut out_buffer, mut sub_buffer) = self.tmp_state.take().unwrap();
            let ref mut input = inputs[0];
            out_buffer.active = input.active;
            (self.callback)(input, &mut out_buffer);
            let out_avail = out_buffer.len();
            copy_out_ring(out_avail, &mut sub_buffer, &mut out_buffer, outputs);
            self.tmp_state = Some((in_buffer, out_buffer, sub_buffer));
        }
        else if outputs.len() == 1 {
            let (mut in_buffer, mut out_buffer, mut sub_buffer) = self.tmp_state.take().unwrap();
            self.base_mix.mix_inputs_ring(inputs, &mut in_buffer);
            let ref mut output = outputs[0];
            output.active = in_buffer.active;
            (self.callback)(&mut in_buffer, output);
            self.tmp_state = Some((in_buffer, out_buffer, sub_buffer));
        }
        else {
            let (mut in_buffer, mut out_buffer, mut sub_buffer) = self.tmp_state.take().unwrap();
            self.base_mix.mix_inputs_ring(inputs, &mut in_buffer);
            out_buffer.active = in_buffer.active;
            (self.callback)(&mut in_buffer, &mut out_buffer);
            let out_avail = out_buffer.len();
            copy_out_ring(out_avail, &mut sub_buffer, &mut out_buffer, outputs);
            self.tmp_state = Some((in_buffer, out_buffer, sub_buffer));
        }
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
