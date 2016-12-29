use super::{RingBuffer};

pub fn copy_out(amount: usize, buffer: &Vec<i16>, outputs: &mut [RingBuffer]) {
    for out in outputs.iter_mut() {
        out.write_from(amount, buffer);
    }
}
