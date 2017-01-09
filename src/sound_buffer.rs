// let sound_buffer = |debug_name, start : usize, start_amount : usize, stop : usize, overrun : usize| {
//     let mut music_buffer = RingBuffer::new();
//     music_buffer.max_length = start * 2 * 2;
//     let mut start_reached = false;
//     let mut music_last = Instant::now();
//     let mut music_disabled = false;
//     let mut sample_error = 0;
//     let mut samples_out = 0;
//
//     Box::new(Callback::new(Box::new(move |input, output| {
//         output.active = false;
//         if music_disabled && Instant::now().duration_since(music_last).subsec_nanos() > 50000000 {
//             music_disabled = false;
//         }
//         if music_disabled {
//             input.clear();
//         }
//         if !music_disabled && input.active {
//             music_buffer.write_from_ring(input.len(), input);
//         }
//         if !music_disabled && input.active && !start_reached && music_buffer.len() > start * 2 {
//             // println!("{} buffer start", debug_name);
//             start_reached = true;
//             music_last = Instant::now();
//             output.write_from_ring(min(music_buffer.len(), start_amount * 2 as usize), &mut music_buffer);
//         }
//         if start_reached && input.active {
//             output.active = true;
//             let since_last = Instant::now();
//             let since = since_last.duration_since(music_last);
//             let samples = (((since.subsec_nanos() + sample_error as u32) as f64 / 1000000.0).floor() as usize) * 48;
//             sample_error = (since.subsec_nanos() as usize + sample_error - samples / 48 * 1000000) as usize;
//             // let samples = max(start_amount - output.len() / 2, 0);
//             // print!("samples {:?} {:?} {:?}", music_buffer.len(), start_amount, output.len());
//             if samples < overrun {
//                 output.write_from_ring(min(music_buffer.len(), samples * 2), &mut music_buffer);
//                 music_last = since_last;
//             }
//             else {
//                 output.active = false;
//                 // println!("{} overrun", debug_name);
//                 output.clear();
//                 music_buffer.clear();
//                 music_disabled = true;
//                 music_last = Instant::now();
//                 sample_error = 0;
//             }
//             if music_buffer.len() <= stop * 2 {
//                 output.active = false;
//                 // println!("empty {} buffer", debug_name);
//                 start_reached = false;
//                 sample_error = 0;
//             }
//         }
//         else if start_reached {
//             sample_error = 0;
//             music_last = Instant::now();
//         }
//         //     let since_last = Instant::now();
//         //     let since = since_last.duration_since(music_last);
//         //     let samples = (((since.subsec_nanos() + sample_error as u32) as f64 / 1000000.0).floor() as usize) * 48;
//         //     sample_error = (since.subsec_nanos() as usize + sample_error - samples / 48 * 1000000) as usize;
//         //     music_last = since_last;
//         // }
//         // print!("{:?} ", output.buffer.as_ptr());
//     })))
// };
