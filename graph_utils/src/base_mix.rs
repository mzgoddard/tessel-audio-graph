use std::cmp::min;
use std::time::{Instant, Duration};

use super::{Node, RingBuffer, copy_out};

pub struct BaseMix {
    pub accum: Vec<i16>,
    read_copy: Vec<i16>,
    // last: Instant,
    // avail_error: usize,
}

impl Default for BaseMix {
    fn default() -> BaseMix {
        BaseMix {
            accum: Vec::<i16>::new(),
            read_copy: Vec::<i16>::new(),
            // last: Instant::now(),
            // avail_error: 0,
        }
    }
}

impl BaseMix {
    pub fn new() -> BaseMix {
        BaseMix { ..Default::default() }
    }

    pub fn mix_inputs(&mut self, inputs: &mut [RingBuffer]) -> usize {
        // let now = Instant::now()
        let avail = {
            // inputs.iter().fold(usize::max_value(), |a, v| min(a, v.len()))
            if inputs.iter().filter(|x| x.active).count() > 0 {
                inputs.iter().filter(|x| x.active).fold(usize::max_value(), |a, v| min(a, v.len()))
            }
            else {
                0
            }
        };
        // print!("{:?} {:?} ", avail, inputs.iter().map(|x| (x.active, x.len())).collect::<Vec<(bool, usize)>>());
        for _ in self.accum.len()..avail {
            self.accum.push(0);
            self.read_copy.push(0);
        }
        for i in 0..avail {
            self.accum[i] = 0;
        }
        let num_inputs = inputs.len();
        // if inputs.len() > 0 {
        //     inputs[0].read_into(avail, &mut self.accum);
        // }
        // for input in inputs.iter_mut().skip(1) {
        //     input.read_into(avail, &mut self.read_copy);
        // }
        // if inputs.len() > 0 {
        //     inputs[num_inputs - 1].read_into(avail, &mut self.accum);
        // }
        // for input in inputs.iter_mut().take(num_inputs - 1) {
        //     input.read_into(avail, &mut self.read_copy);
        // }
        for input in inputs.iter_mut() {
            if input.active && input.len() > 0 {
                assert!(input.len() >= avail);
                for i in 0..avail {
                    self.read_copy[i] = 0;
                }
                input.read_into(avail, &mut self.read_copy);
                for i in 0..avail {
                    self.accum[i] += self.read_copy[i];
                    // self.accum[i] = (self.accum[i] + self.read_copy[i]) / 2;
                    // self.accum[i] = (((self.accum[i] + self.read_copy[i]) as f32 / 32767.0).tanh() * 32767.0) as i16;
                }
            }
        }
        avail
    }

    pub fn mix_inputs_ring(&mut self, inputs: &mut [RingBuffer], ring: &mut RingBuffer) {
        let avail = self.mix_inputs(inputs);
        ring.active = inputs.len() > 0 && inputs.iter().any(|x| x.active);
        ring.write_from(avail, &self.accum);
    }
}

impl Node for BaseMix {
    fn update(&mut self, inputs: &mut [RingBuffer], outputs: &mut [RingBuffer]) {
        copy_out(self.mix_inputs(inputs), &mut self.accum, outputs);
        let active = inputs.len() > 0 && inputs.iter().any(|x| x.active);
        for output in outputs.iter_mut() {
            output.active = active
        }
    }
}

#[cfg(test)]
mod test {
    use super::BaseMix;
    use super::super::{Node, RingBuffer};

    #[test]
    fn it_mixes_input() {
        let mut a = BaseMix::new();
        let v1 = (48..96).map(|x| x as i16).collect::<Vec<i16>>();
        let v2 = (96..144).map(|x| x as i16).collect::<Vec<i16>>();
        let mut inputs = vec!(RingBuffer::new(), RingBuffer::new());
        inputs[0].write_from(v1.len(), &v1);
        inputs[1].write_from(v2.len(), &v2);
        {
            let n = &mut a as &mut Node;
            n.update(&mut inputs, &mut []);
        }
        assert_eq!(a.accum[0], 144);
    }
}
