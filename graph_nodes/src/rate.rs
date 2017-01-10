use graph_utils::{Callback, CallbackInner, RingBuffer};

pub struct Rate(Callback);

impl CallbackInner for Rate {
    fn get_callback(&mut self) -> &mut Callback {
        &mut self.0
    }
}

impl Rate {
    pub fn new(input_rate: usize, output_rate: usize) -> Box<Rate> {
        let input_upper = input_rate * 2 / 1000;
        let input_lower = input_rate * 2 % 1000;
        let input_upper_max = input_upper + if input_lower != 0 {1} else {0};
        let mut input_carry = 0;
        let output_upper = output_rate * 2 / 1000;
        let output_lower = output_rate * 2 % 1000;
        let output_upper_max = output_upper + if output_lower != 0 {1} else {0};
        let mut output_carry = 0;
        Box::new(Rate(Callback::new(Box::new(move |input, output| {
            let mut avail = input.len();

            let mut num = 0;
            let mut denom = 0;
            while num + input_upper_max <= avail {
                num += input_upper;
                input_carry += input_lower;
                if input_carry >= 2000 {
                    let diff = input_carry / 2000;
                    input_carry -= diff * 2000;
                    num += diff * 2;
                }
                denom += output_upper;
                output_carry += output_lower;
                if output_carry >= 2000 {
                    let diff = output_carry / 2000;
                    output_carry -= diff * 2000;
                    denom += diff * 2;
                }
            }

            let slice = input.read_slice(num);
            let mut out_slice = output.write_slice(denom);

            for (index, o) in out_slice.iter_mut().enumerate() {
                *o = slice[index / 2 * num / denom * 2 + index % 2];
            }
        }))))
    }
}

// let r48_to_r44 = || {
//     let mut frames = 9;
//     Box::new(Callback::new(Box::new(move |input, output| {
//         let mut avail = input.len();
//
//         let mut num = 0;
//         let mut denom = 0;
//         while num + 96 <= avail {
//             num += 96;
//             denom += 88;
//             frames += 1;
//             if frames == 10 {
//                 // avail += 2;
//                 denom += 2;
//                 frames = 0;
//             }
//         }
//
//         let slice = input.read_slice(num);
//         let mut out_slice = output.write_slice(denom);
//
//         for (index, o) in out_slice.iter_mut().enumerate() {
//             *o = slice[index / 2 * num / denom * 2 + index % 2];
//         }
//     })))
// };
