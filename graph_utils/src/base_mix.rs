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
            // self.read_copy.push(0);
        }
        for i in 0..avail {
            self.accum[i] = 0;
        }
        let num_inputs = inputs.iter().filter(|x| x.active).count();
        // if num_inputs == 1 {
        //
        // }
        // else {
            for input in inputs.iter_mut() {
                if input.active && input.len() > 0 {
                    assert!(input.len() >= avail);
                    // for i in 0..avail {
                    //     self.read_copy[i] = 0;
                    // }
                    let slice = input.read_slice(avail);
                    // input.read_into(avail, &mut self.read_copy);
                    for i in 0..avail {
                        self.accum[i] += slice[i];
                    }
                }
            }
        // }
        avail
    }

    pub fn mix_inputs_ring(&mut self, inputs: &mut [RingBuffer], ring: &mut RingBuffer) {
        let num_inputs = inputs.iter().filter(|x| x.active).count();
        if num_inputs == 0 {
            ring.active = false;
            return;
        }
        else if num_inputs == 1 {
            let mut input = inputs.iter_mut().filter(|x| x.active).nth(0).unwrap();
            let avail = input.len();
            ring.active = input.active;
            ring.write_from_ring(avail, input);
        }
        else {
            let avail = self.mix_inputs(inputs);
            ring.active = inputs.len() > 0 && inputs.iter().any(|x| x.active);
            ring.write_from(avail, &self.accum);
        }
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
