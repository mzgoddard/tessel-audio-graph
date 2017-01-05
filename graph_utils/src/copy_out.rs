use super::{RingBuffer};

pub fn copy_out(amount: usize, buffer: &Vec<i16>, outputs: &mut [RingBuffer]) {
    for out in outputs.iter_mut() {
        out.write_from(amount, buffer);
        // print!("{:?} ", out.buffer.as_ptr());
    }
}

pub fn copy_out_ring(amount: usize, buffer: &mut Vec<i16>, ring: &mut RingBuffer, outputs: &mut [RingBuffer]) {
    // let num_outputs = outputs.iter().filter(|x| x.active).count();
    // if num_outputs == 0 {
    //     return;
    // }
    // else if num_outputs == 1 {
    //     let output = outputs.iter_mut().filter(|x| x.active).nth(0).unwrap();
    //     let avail = ring.len();
    //     output.active = ring.active;
    //     output.write_from_ring(avail, ring);
    // }
    // else {
        // ring.read_into(amount, buffer);
        let active = ring.active;
        let slice = ring.read_slice(amount);
        for out in outputs.iter_mut() {
            out.active = active;
            out.write_from_read_slice(amount, &slice);
            // print!("{:?} ", out.buffer.as_ptr());
        }
    // }
}
