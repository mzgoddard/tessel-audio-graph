use std::sync::{Arc, Mutex, Condvar};
use std::time::Instant;
use std::io;
use std::io::{ErrorKind, Read};
use std::slice;

use graph_utils::{Node, RingBuffer, Capture};

use activation::*;

pub struct IoNodeBuffer {
    name: &'static str,
    activation_controller: ActivationController,
    stream_mutex: Arc<Mutex<RingBuffer>>,
    last_tick: Instant,
    reader_tick: Arc<(Mutex<i32>, Condvar, Mutex<bool>, Condvar)>,
    should_shutdown: Arc<Mutex<bool>>,
}

pub struct IoReadFactory {
    name: &'static str,
    stream_mutex: Arc<Mutex<RingBuffer>>,
    reader_tick: Arc<(Mutex<i32>, Condvar, Mutex<bool>, Condvar)>,
    should_shutdown: Arc<Mutex<bool>>,
}

pub struct IoCapture(Capture);

impl Node for IoCapture {
    fn update(&mut self, inputs: &mut [RingBuffer], outputs: &mut [RingBuffer]) {
        self.0.update(inputs, outputs);
    }
}

pub type IoReader = Box<Fn(&mut io::Read) + Send + Sync>;

impl Drop for IoNodeBuffer {
    fn drop(&mut self) {
        if let Ok(mut guard) = self.should_shutdown.lock() {
            *guard = true;
        }
        self.reader_tick.1.notify_all();
    }
}

impl IoNodeBuffer {
    pub fn new(name: &'static str, activation_controller: ActivationController) -> IoNodeBuffer {
        IoNodeBuffer {
            name: name,
            activation_controller: activation_controller,
            stream_mutex: Arc::new(Mutex::new(RingBuffer::new())),
            last_tick: Instant::now(),
            reader_tick: Arc::new((Mutex::new(-1), Condvar::new(), Mutex::new(false), Condvar::new())),
            should_shutdown: Arc::new(Mutex::new(false)),
        }
    }

    pub fn update(&mut self, now: Instant) {
        if now.duration_since(self.last_tick).subsec_nanos() > 1000000 {
            self.last_tick = now;
            self.reader_tick.1.notify_one();
        }
    }

    pub fn capture(&self) -> Box<IoCapture> {
        let name = self.name;
        let stream_mutex = self.stream_mutex.clone();
        let activation_controller_clone = self.activation_controller.clone();
        let mut activation_guard = None;

        let mut last_received = Instant::now();
        let mut state = 0;
        let mut paused = false;
        let mut samples = 0;

        Box::new(IoCapture(Capture::new(Box::new(move |output| {
            let music_len = {
                if let Ok(mut http_music) = (*stream_mutex).try_lock() {
                    let len = http_music.len();
                    if state != 2 {
                        http_music.clear();
                    }
                    len
                }
                else {
                    0
                }
            };

            output.active = false;

            if state == 0 && music_len > 0 {
                activation_guard = activation_controller_clone.activate();
                if activation_guard.is_some() {
                    println!("activating http {}", name);
                    last_received = Instant::now();
                    samples = 0;
                    state = 1;
                }
            }
            else if state == 1 && music_len > 0 && samples > 192000 {
                activation_guard = None;
                println!("activated http {}", name);
                samples = 0;
                state = 2;
            }
            else if state == 1 && music_len == 0 && Instant::now().duration_since(last_received).as_secs() > 3 {
                activation_guard = None;
                println!("didn't activate http {}", name);
                state = 0;
            }
            else if state == 1 && music_len > 0 {
                last_received = Instant::now();
                samples += music_len;
            }
            else if state == 1 {
                // samples += 20;
            }
            else if state == 2 && music_len > 0 {
                last_received = Instant::now();
                samples = 0;

                match activation_controller_clone.is_activating() {
                    ActivationState::Activating => {
                        if !paused {
                            println!("paused http {}", name);
                        }
                        paused = true;
                    },
                    ActivationState::Running => {
                        if paused {
                            println!("resumed http {}", name);
                        }
                        paused = false;
                    },
                    _ => {},
                }

                output.active = !paused;

                if let Ok(mut http_music) = (*stream_mutex).try_lock() {
                    if !paused {
                        output.write_from_ring((*http_music).len(), &mut *http_music);
                    }
                    else {
                        (*http_music).clear();
                    }
                }
            }
            else if state == 2 && music_len == 0 && samples < 48000 {
                // samples += 20;
                output.active = !paused;
            }
            else if state == 2 && music_len == 0 && Instant::now().duration_since(last_received).as_secs() >= 2 {
                state = 0;
            }
        }))))
    }

    pub fn read_factory(&self) -> IoReadFactory {
        IoReadFactory {
            name: self.name,
            stream_mutex: self.stream_mutex.clone(),
            reader_tick: self.reader_tick.clone(),
            should_shutdown: self.should_shutdown.clone(),
        }
    }
}

impl IoReadFactory {
    pub fn reader(&self) -> Box<Fn(&mut io::Read) + Send + Sync> {
        let name = self.name;
        let stream_mutex = self.stream_mutex.clone();
        let should_shutdown = self.should_shutdown.clone();
        let net_tick_pair = self.reader_tick.clone();

        Box::new(move |stream: &mut Read| {
            // if let request::Body(reader) = req.body {
            //     match reader {
            //         HttpReader::EofReader(reader) => {
            //             reader.set_read_timeout(Duration::from_millis(1));
            //         },
            //         _ => {},
            //     }
            // }

            println!("{}", name);
            let mut buffer = Vec::<i16>::new();
            for _ in 0..16384 {
                buffer.push(0);
            }
            let mut inner = RingBuffer::new();

            // sleep(Duration::from_millis(20));
            let mut sample_error = 0;
            let mut samples_missed = 0;

            let &(ref net_mutex, ref net_condvar, ref net_out_mutex, ref net_out_condvar) = &*net_tick_pair;
            let mut net_guard = net_mutex.lock().unwrap();
            if *net_guard != -1 {
                println!("{} already connected", name);
                return;
            }
            *net_guard = 0;
            // {
            //     let mut net_out_guard = net_out_mutex.lock().unwrap();
            //     *net_out_guard = true;
            // }
            // println!("{} wait", name);
            net_guard = net_condvar.wait(net_guard).unwrap();

            let mut last_read = Instant::now();
            loop {
                if let Ok(should_shutdown) = should_shutdown.lock() {
                    if *should_shutdown {
                        break;
                    }
                }

                let start = Instant::now();
                let mut did_error = false;

                loop {
                    let samples = 192;
                    match unsafe {
                        let buffer_ptr = buffer.as_mut_ptr();
                        let mut slice = slice::from_raw_parts_mut(buffer_ptr as *mut u8, samples as usize * 2 * 2);
                        stream.read(slice)
                    } {
                        Ok(read) => {
                            let now = Instant::now();
                            if read > 0 {
                                if let Ok(mut http_music) = stream_mutex.try_lock() {
                                    last_read = now;
                                    http_music.write_from(read / 2, &buffer);
                                }
                                else {
                                    inner.write_from(read / 2, &buffer);
                                }
                            }
                            if now.duration_since(start).subsec_nanos() > 500000 {
                                // print!("read for 2ms ");
                                break;
                            }
                            // if samples_received > should_received {
                            //     // print!("read enough samples ");
                            //     break;
                            // }
                            // if read / 2 / 2 < samples as usize {
                            if read == 0 {
                                // print!("less than expected {} {} ", read / 2 / 2, samples);
                                break;
                            }
                        },
                        Err(err) => {
                            if err.kind() == ErrorKind::WouldBlock {
                                break;
                            }
                            did_error = true;
                            if let Ok(mut http_music) = (*stream_mutex).lock() {
                                (*http_music).clear();
                                // print!("read{:?}", read / 2 / 2);
                            }
                            println!("break {:?} error: {:?}", name, err);
                            break;
                        },
                    }
                }
                // print!("{} {} {} ", name, should_received, samples_received);
                if did_error {
                    break;
                }
                if Instant::now().duration_since(last_read).as_secs() > 1 {
                // if samples_missed > 48000 {
                    samples_missed = 0;
                    if let Ok(mut http_music) = (*stream_mutex).lock() {
                        (*http_music).clear();
                        // print!("read{:?}", read / 2 / 2);
                    }
                    println!("break {:?} no data after a second", name);
                    break;
                }
                // else {
                //     samples_missed += 1;
                // }
                if inner.len() > 0 {
                    // println!("sending {} upstream", name);
                    // samples_missed = 0;
                    last_read = Instant::now();
                    if let Ok(mut http_music) = (*stream_mutex).lock() {
                        if (*http_music).len() < 16384 {
                            (*http_music).write_from_ring(inner.len(), &mut inner);
                        }
                        else {
                            println!("cleaning build up in {:?} stream", name);
                            inner.clear();
                        }
                        // print!("read{:?}", read / 2 / 2);
                    }
                    else {
                        println!("couldn't lock upstream channel");
                        break;
                    }
                }

                // {
                //     let net_out_guard = net_out_mutex.lock().unwrap();
                //     net_out_condvar.notify_one();
                // }
                // println!("{} wait", name);
                net_guard = net_condvar.wait(net_guard).unwrap();
            }

            // {
            //     let mut net_out_guard = net_out_mutex.lock().unwrap();
            //     *net_out_guard = false;
            //     net_out_condvar.notify_one();
            // }
            *net_guard = -1;
        })
    }
}

// fn net_capture(net_stream_mutex: Arc<Mutex<RingBuffer>>) -> Box<Capture> {
//     // let http_music_mutex = Arc::new(Mutex::new(RingBuffer::new()));
//     // let http_music_mutex_clone = http_music_mutex.clone();
//     // let tcp_music_mutex_clone = http_music_mutex.clone();
//     // let net_music_tick_pair = Arc::new((Mutex::new(-1), Condvar::new(), Mutex::new(false), Condvar::new()));
//     // let net_music_tick_pair_node = net_music_tick_pair.clone();
//
//     let activation_controller_clone = activation_controller.clone();
//     let mut activation_guard = None;
//
//     let mut last_received = Instant::now();
//     let mut state = 0;
//     let mut paused = false;
//     let mut samples = 0;
//
//     Box::new(Capture::new(Box::new(move |output| {
//         let music_len = {
//             if let Ok(mut http_music) = (*net_stream_mutex).try_lock() {
//                 let len = http_music.len();
//                 if state != 2 {
//                     http_music.clear();
//                 }
//                 len
//             }
//             else {
//                 0
//             }
//         };
//
//         output.active = false;
//
//         if state == 0 && music_len > 0 {
//             activation_guard = activation_controller_clone.activate();
//             if activation_guard.is_some() {
//                 println!("activating http music");
//                 samples = 0;
//                 state = 1;
//             }
//         }
//         else if state == 1 && music_len > 0 && samples > 192000 {
//             activation_guard = None;
//             println!("activated http music");
//             samples = 0;
//             state = 2;
//         }
//         else if state == 1 && music_len == 0 && samples > 288000 {
//             activation_guard = None;
//             println!("didn't activate http music");
//             state = 0;
//         }
//         else if state == 1 && music_len > 0 {
//             samples += music_len;
//         }
//         else if state == 1 {
//             samples += 20;
//         }
//         else if state == 2 && music_len > 0 {
//             samples = 0;
//
//             match activation_controller_clone.is_activating() {
//                 ActivationState::Activating => {
//                     if !paused {
//                         println!("paused http music");
//                     }
//                     paused = true;
//                 },
//                 ActivationState::Running => {
//                     if paused {
//                         println!("resumed http music");
//                     }
//                     paused = false;
//                 },
//                 _ => {},
//             }
//
//             output.active = !paused;
//
//             if let Ok(mut http_music) = (*net_stream_mutex).try_lock() {
//                 if !paused {
//                     output.write_from_ring((*http_music).len(), &mut *http_music);
//                 }
//                 else {
//                     (*http_music).clear();
//                 }
//             }
//         }
//         else if state == 2 && music_len == 0 && samples < 48000 {
//             samples += 20;
//             output.active = !paused;
//         }
//         else if state == 2 && music_len == 0 && samples >= 48000 {
//             state = 0;
//         }
//     })))
// };
//
// fn audio_stream_factory(name: &'static str, stream_mutex: Arc<Mutex<RingBuffer>>, should_shutdown_mutex: Arc<Mutex<bool>>, net_tick_pair: Arc<(Mutex<i32>, Condvar, Mutex<bool>, Condvar)>) -> Box<Fn(&mut Read) + Send + Sync> {
//     Box::new(move |stream: &mut Read| {
//         // if let request::Body(reader) = req.body {
//         //     match reader {
//         //         HttpReader::EofReader(reader) => {
//         //             reader.set_read_timeout(Duration::from_millis(1));
//         //         },
//         //         _ => {},
//         //     }
//         // }
//
//         println!("{}", name);
//         let mut buffer = Vec::<i16>::new();
//         for _ in 0..16384 {
//             buffer.push(0);
//         }
//         let mut inner = RingBuffer::new();
//
//         // sleep(Duration::from_millis(20));
//         let mut sample_error = 0;
//         let mut samples_missed = 0;
//
//         let &(ref net_mutex, ref net_condvar, ref net_out_mutex, ref net_out_condvar) = &*net_tick_pair;
//         let mut net_guard = net_mutex.lock().unwrap();
//         if *net_guard != -1 {
//             println!("{} already connected", name);
//             return;
//         }
//         *net_guard = 0;
//         // {
//         //     let mut net_out_guard = net_out_mutex.lock().unwrap();
//         //     *net_out_guard = true;
//         // }
//         // println!("{} wait", name);
//         net_guard = net_condvar.wait(net_guard).unwrap();
//
//         let mut last_read = Instant::now();
//         loop {
//             if let Ok(should_shutdown) = should_shutdown_mutex.lock() {
//                 if *should_shutdown {
//                     break;
//                 }
//             }
//
//             let start = Instant::now();
//             let mut did_error = false;
//
//             loop {
//                 let samples = 192;
//                 match unsafe {
//                     let buffer_ptr = buffer.as_mut_ptr();
//                     let mut slice = slice::from_raw_parts_mut(buffer_ptr as *mut u8, samples as usize * 2 * 2);
//                     stream.read(slice)
//                 } {
//                     Ok(read) => {
//                         let now = Instant::now();
//                         if read > 0 {
//                             if let Ok(mut http_music) = stream_mutex.try_lock() {
//                                 last_read = now;
//                                 http_music.write_from(read / 2, &buffer);
//                             }
//                             else {
//                                 inner.write_from(read / 2, &buffer);
//                             }
//                         }
//                         if now.duration_since(start).subsec_nanos() > 500000 {
//                             // print!("read for 2ms ");
//                             break;
//                         }
//                         // if samples_received > should_received {
//                         //     // print!("read enough samples ");
//                         //     break;
//                         // }
//                         // if read / 2 / 2 < samples as usize {
//                         if read == 0 {
//                             // print!("less than expected {} {} ", read / 2 / 2, samples);
//                             break;
//                         }
//                     },
//                     Err(err) => {
//                         if err.kind() == ErrorKind::WouldBlock {
//                             break;
//                         }
//                         did_error = true;
//                         if let Ok(mut http_music) = (*stream_mutex).lock() {
//                             (*http_music).clear();
//                             // print!("read{:?}", read / 2 / 2);
//                         }
//                         println!("break {:?} error: {:?}", name, err);
//                         break;
//                     },
//                 }
//             }
//             // print!("{} {} {} ", name, should_received, samples_received);
//             if did_error {
//                 break;
//             }
//             if Instant::now().duration_since(last_read).as_secs() > 1 {
//             // if samples_missed > 48000 {
//                 samples_missed = 0;
//                 if let Ok(mut http_music) = (*stream_mutex).lock() {
//                     (*http_music).clear();
//                     // print!("read{:?}", read / 2 / 2);
//                 }
//                 println!("break {:?} no data after a second", name);
//                 break;
//             }
//             // else {
//             //     samples_missed += 1;
//             // }
//             if inner.len() > 0 {
//                 // println!("sending {} upstream", name);
//                 // samples_missed = 0;
//                 last_read = Instant::now();
//                 if let Ok(mut http_music) = (*stream_mutex).lock() {
//                     if (*http_music).len() < 16384 {
//                         (*http_music).write_from_ring(inner.len(), &mut inner);
//                     }
//                     else {
//                         println!("cleaning build up in {:?} stream", name);
//                         inner.clear();
//                     }
//                     // print!("read{:?}", read / 2 / 2);
//                 }
//                 else {
//                     println!("couldn't lock upstream channel");
//                     break;
//                 }
//             }
//
//             // {
//             //     let net_out_guard = net_out_mutex.lock().unwrap();
//             //     net_out_condvar.notify_one();
//             // }
//             // println!("{} wait", name);
//             net_guard = net_condvar.wait(net_guard).unwrap();
//         }
//
//         // {
//         //     let mut net_out_guard = net_out_mutex.lock().unwrap();
//         //     *net_out_guard = false;
//         //     net_out_condvar.notify_one();
//         // }
//         *net_guard = -1;
//     })
// }
