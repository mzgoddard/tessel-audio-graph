use super::{RingBuffer};

pub fn copy_out(amount: usize, buffer: &Vec<i16>, outputs: &mut [RingBuffer]) {
    for out in outputs.iter_mut() {
        out.write_from(amount, buffer);
        // print!("{:?} ", out.buffer.as_ptr());
    }
}

pub fn copy_out_ring(amount: usize, buffer: &mut Vec<i16>, ring: &mut RingBuffer, outputs: &mut [RingBuffer]) {
    ring.read_into(amount, buffer);
    for out in outputs.iter_mut() {
        out.active = ring.active;
        out.write_from(amount, buffer);
        // print!("{:?} ", out.buffer.as_ptr());
    }
}
