use std::cmp::min;

use super::{Node, RingBuffer, copy_out};

pub struct BaseMix {
    pub accum: Vec<i16>,
    read_copy: Vec<i16>,
}

impl Default for BaseMix {
    fn default() -> BaseMix {
        BaseMix {
            accum: Vec::<i16>::new(),
            read_copy: Vec::<i16>::new(),
        }
    }
}

impl BaseMix {
    pub fn new() -> BaseMix {
        BaseMix { ..Default::default() }
    }

    pub fn mix_inputs(&mut self, inputs: &mut [RingBuffer]) -> usize {
        let avail = {
            if inputs.iter().filter(|x| x.len() > 0).count() > 0 {
                inputs.iter().filter(|x| x.len() > 0).fold(usize::max_value(), |a, v| min(a, v.len()))
            }
            else {
                0
            }
        };
        for _ in self.accum.len()..avail {
            self.accum.push(0);
            self.read_copy.push(0);
        }
        for i in 0..avail {
            self.accum[i] = 0;
        }
        for input in inputs.iter_mut() {
            if input.len() > 0 {
                input.read_into(avail, &mut self.read_copy);
                for i in 0..avail {
                    self.accum[i] += self.read_copy[i];
                }
            }
        }
        avail
    }
}

impl Node for BaseMix {
    fn update(&mut self, inputs: &mut [RingBuffer], outputs: &mut [RingBuffer]) {
        copy_out(self.mix_inputs(inputs), &mut self.accum, outputs);
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
