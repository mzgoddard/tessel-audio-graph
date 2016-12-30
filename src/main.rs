//! A blinky example for Tessel

// Import the tessel library
extern crate tessel;
// Import the libusb library
extern crate libusb;
// Import the alsa library
extern crate alsa;
// Import the graph_utils library
extern crate graph_utils;
// Import the http server library
extern crate iron;
// Import the router library
#[macro_use(router)]
extern crate router;

// Import the Tessel API
use tessel::Tessel;
// Import sleep from the standard lib
use std::thread::{sleep, yield_now};
// Import durations from the standard lib
use std::time::{Duration, Instant};

use std::cmp::{min, max};

use std::string::String;

use std::thread;
use std::sync::*;
use std::slice;
use std::io::Read;

use std::ffi::CString;
use alsa::device_name::HintIter;
use alsa::card;

use alsa::{Direction, ValueOr};
use alsa::pcm::{PCM, HwParams, SwParams, Format, Access, State};

use graph_utils::*;

use iron::prelude::*;
use iron::status;

use router::*;

struct AlsaHwParams {
    channels: u32,
    rate: u32,
    format: Format,
    access: Access,
    periods: u32,
    period_size: i32,
}

impl Default for AlsaHwParams {
    fn default() -> AlsaHwParams {
        AlsaHwParams {
            channels: 2,
            rate: 48000,
            format: Format::s16(),
            access: Access::RWInterleaved,
            periods: 2,
            period_size: 48,
        }
    }
}

impl AlsaHwParams {
    fn set_params(&self, pcm: &PCM) {
        let hwp = HwParams::any(&pcm).unwrap();
        hwp.set_channels(self.channels).unwrap();
        hwp.set_rate(self.rate, ValueOr::Nearest).unwrap();
        hwp.set_format(self.format).unwrap();
        hwp.set_access(Access::RWInterleaved).unwrap();
        hwp.set_periods(self.periods, ValueOr::Nearest).unwrap();
        hwp.set_period_size_near(self.period_size, ValueOr::Nearest).unwrap();
        println!("{:?}", hwp);
        pcm.hw_params(&hwp).unwrap();
    }
}

struct AlsaSwParams {
    avail_min: i32,
    start_threshold: i32,
}

impl Default for AlsaSwParams {
    fn default() -> AlsaSwParams {
        AlsaSwParams {
            avail_min: 0,
            start_threshold: 0,
        }
    }
}

impl AlsaSwParams {
    fn set_params(&self, pcm: &PCM) {
        let swp = pcm.sw_params_current().unwrap();
        swp.set_avail_min(self.avail_min).unwrap();
        swp.set_start_threshold(self.start_threshold).unwrap();
        println!("{:?}", swp);
        pcm.sw_params(&swp).unwrap();
    }
}

struct AlsaCard {
    pcm_name: &'static str,
    pcm_hint: &'static str,
    hw_params: AlsaHwParams,
    sw_params: AlsaSwParams,
}

impl Default for AlsaCard {
    fn default() -> AlsaCard {
        AlsaCard {
            pcm_name: "default",
            pcm_hint: "default",
            hw_params: Default::default(),
            sw_params: Default::default(),
        }
    }
}

fn main() {
    // Create a new Tessel
    let mut tessel = Tessel::new();

    // Turn on one of the LEDs
    // tessel.led[2].on().unwrap();

    println!("I'm blinking! (Press CTRL + C to stop)");

    let mut context = libusb::Context::new().unwrap();

    for mut device in context.devices().unwrap().iter() {
        let device_desc = device.device_descriptor().unwrap();

        println!("Bus {:03} Device {:03} ID {:04x}:{:04x}:{:02x}",
            device.bus_number(),
            device.address(),
            device_desc.vendor_id(),
            device_desc.product_id(),
            device_desc.class_code());
    }

    for t in &["pcm"] {
        println!("{} devices:", t);
        let i = HintIter::new(None, &*CString::new(*t).unwrap()).unwrap();
        for a in i { println!("  {:?}", a) }
    }

    // loop {
    //     for c in alsa::card::Iter::new() {
    //         if let Ok(_c) = c {
    //             println!("{:?} {:?}", _c.get_name(), _c.get_longname());
    //         }
    //     }
    //     println!("...");
    //     sleep(Duration::from_millis(1000));
    // }

    // {
    //     // Open default playback device
    //     let pcm = PCM::open(&*CString::new("default").unwrap(), Direction::Playback, false).unwrap();
    //     let pcm2 = PCM::open(&*CString::new("default").unwrap(), Direction::Playback, false).unwrap();
    //     // let pcm3 = PCM::open(&*CString::new("default").unwrap(), Direction::Playback, false).unwrap();
    //
    //     // Set hardware parameters: 44100 Hz / Mono / 16 bit
    //     let hwp = HwParams::any(&pcm).unwrap();
    //     hwp.set_channels(1).unwrap();
    //     hwp.set_rate(44100, ValueOr::Nearest).unwrap();
    //     hwp.set_format(Format::s16()).unwrap();
    //     hwp.set_access(Access::RWInterleaved).unwrap();
    //     pcm.hw_params(&hwp).unwrap();
    //     pcm2.hw_params(&hwp).unwrap();
    //     // pcm3.hw_params(&hwp).unwrap();
    //     let io = pcm.io_i16().unwrap();
    //     let io2 = pcm2.io_i16().unwrap();
    //     // let io3 = pcm3.io_i16().unwrap();
    //
    //     // Make a sine wave
    //     let mut buf = [0i16; 1024];
    //     for (i, a) in buf.iter_mut().enumerate() {
    //         *a = ((i as f32 * 2.0 * ::std::f32::consts::PI / 128.0).sin() * 8192.0 * 3.0) as i16
    //     }
    //     let mut buf2 = [0i16; 1000];
    //     for (i, a) in buf2.iter_mut().enumerate() {
    //         *a = ((i as f32 * 2.0 * ::std::f32::consts::PI / 125.0).sin() * 8192.0 * 3.0) as i16
    //     }
    //
    //     // Play it back for 2 seconds.
    //     for _ in 0..2*44100/1024 {
    //         assert_eq!(io.writei(&buf[..]).unwrap(), 1024);
    //     }
    //     for _ in 0..2*44100/1000 {
    //         assert_eq!(io.writei(&buf2[..]).unwrap(), 1000);
    //     }
    //
    //     // In case the buffer was larger than 2 seconds, start the stream manually.
    //     if pcm.state() != State::Running { pcm.start().unwrap() };
    //     if pcm2.state() != State::Running { pcm2.start().unwrap() };
    //     // if pcm3.state() != State::Running { pcm3.start().unwrap() };
    //     // Wait for the stream to finish playback.
    //     pcm.drain().unwrap();
    //     pcm2.drain().unwrap();
    //     // pcm3.drain().unwrap();
    // }

    {
        let mut graph = Graph::new();

        let hint_mutex = Arc::new(Mutex::new(Vec::<String>::new()));
        let hint_mutex_clone = hint_mutex.clone();

        thread::spawn(move || {
            // let mut buffer = Vec::new();
            loop {
                // for _ in 0..buffer.len() {
                //     buffer.pop().unwrap();
                // }

                // let now = Instant::now();
                // let mut step_start = Instant::now();
                // for card_result in card::Iter::new() {
                //     if let Ok(card) = card_result {
                //         if let Ok(ref name) = card.get_name() {
                //             buffer.push(name.clone());
                //         }
                //         else {
                //             break;
                //         }
                //     }
                //     else {
                //         break;
                //     }
                //     // println!("step {:?}", Instant::now().duration_since(step_start).subsec_nanos());
                //     yield_now();
                //     // step_start = Instant::now();
                // }
                // println!("search {:?}", Instant::now().duration_since(now).subsec_nanos());

                // for x in HintIter::new(None, &*CString::new("pcm").unwrap()).unwrap() {
                //     if let Some(ref name) = x.name {
                //         buffer.push(name.clone());
                //     }
                //     yield_now();
                // }

                // println!("cards {:?}", buffer);

                if let Ok(mut hints) = hint_mutex_clone.lock() {
                    for _ in 0..(*hints).len() {
                        (*hints).pop().unwrap();
                    }
                    for card_result in card::Iter::new() {
                        if let Ok(card) = card_result {
                            if let Ok(ref name) = card.get_name() {
                                (*hints).push(name.clone());
                            }
                            else {
                                break;
                            }
                        }
                        else {
                            break;
                        }
                        // println!("step {:?}", Instant::now().duration_since(step_start).subsec_nanos());
                        yield_now();
                        // step_start = Instant::now();
                    }

                    // for _ in (*hints).len()..buffer.len() {
                    //     (*hints).push(String::new());
                    // }
                    // for _ in buffer.len()..(*hints).len() {
                    //     (*hints).pop().unwrap();
                    // }
                    // (*hints).clone_from_slice(buffer.as_slice());
                }

                sleep(Duration::from_millis(2500));
            }
        });

        let alsa_playback = |card: AlsaCard| {
            let mut maybe_pcm_io = None;
            let mut buffer = Vec::new();
            let pcm_period = card.hw_params.period_size as usize;
            let pcm_max = pcm_period * card.hw_params.periods as usize;
            let hint_mutex_playback = hint_mutex.clone();
            let mut cooloff = false;
            let mut cooloff_start = Instant::now();

            Box::new(Playback::new(Box::new(move |input| {
                if maybe_pcm_io.is_none() {
                    // let hint = HintIter::new(None, &*CString::new("pcm").unwrap()).unwrap().find(|x| {
                    //     if let Some(ref name) = x.name {
                    //         name == card.pcm_hint
                    //     }
                    //     else {
                    //         false
                    //     }
                    // });
                    if cooloff {
                        if Instant::now().duration_since(cooloff_start).as_secs() > 1 {
                            cooloff = false;
                        }
                        else {
                            return;
                        }
                    }

                    let hint = if let Ok(mut hints) = hint_mutex_playback.try_lock() {
                        if let Some(hint) = (*hints).iter().find(|hint| {
                            **hint == card.pcm_hint
                        }).map(|x| x.clone()) {
                            for _ in 0..hints.len() {
                                (*hints).pop().unwrap();
                            }
                            Some(hint)
                        }
                        else {
                            None
                        }
                    }
                    else {
                        None
                    };

                    if hint.is_some() {
                        if let Ok(mut pcm) = PCM::open(&*CString::new(card.pcm_name).unwrap(), Direction::Playback, true) {
                            card.hw_params.set_params(&pcm);
                            card.sw_params.set_params(&pcm);

                            println!("connect {:?} playback", card.pcm_name);

                            input.clear();
                            maybe_pcm_io = Some(pcm);
                        }
                    }
                }
                let mut unset = false;
                if let Some(ref mut pcm) = maybe_pcm_io {
                    if let Ok(status) = pcm.status() {
                        if status.get_state() == State::Disconnected {
                            println!("disconnected {:?} playback", card.pcm_name);
                            unset = true;
                        }
                        else if status.get_state() == State::XRun {
                            println!("overrun {:?} playback", card.pcm_name);
                            unset = true;
                        }
                    }
                    else {
                        unset = true;
                    }
                }
                if unset {
                    maybe_pcm_io = None;
                    cooloff = true;
                    cooloff_start = Instant::now();
                }
                if let Some(ref mut pcm) = maybe_pcm_io {
                    let mut buffer_avail = input.len() / 2;
                    if let Ok(pcm_avail) = pcm.avail().map(|x| x as usize) {
                        if pcm_max - pcm_avail < pcm_period {
                            buffer_avail += pcm_max - pcm_avail;
                            for _ in buffer.len()..((pcm_max - pcm_avail) * 2) {
                                buffer.push(0);
                            }
                            for i in 0..((pcm_max - pcm_avail) * 2) {
                                buffer[i] = 0;
                            }
                            input.write_from((pcm_max - pcm_avail) * 2, &buffer);
                        }
                        let avail = min(pcm_avail, buffer_avail);

                        if avail > 0 {
                            input.read_into(avail * 2, &mut buffer);
                            if let Ok(io) = pcm.io_i16() {
                                if let Err(_) = io.writei(&buffer[..(avail * 2)]) {
                                    println!("error writing {:?} playback", card.pcm_name);
                                    unset = true;
                                }
                                // print!("p{:?}", avail);
                            }
                            else {
                                println!("error creating io {:?} playback", card.pcm_name);
                                unset = true;
                            }
                        }
                        else {
                            // print!("pcm{:?}buffer{:?}", pcm_avail, buffer_avail);
                            // print!("p0");
                        }
                    }
                    else {
                        println!("error checking avail {:?} playback", card.pcm_name);
                        unset = true;
                    }
                }
                if unset {
                    maybe_pcm_io = None;
                    cooloff = true;
                    cooloff_start = Instant::now();
                }
            })))
        };

        let alsa_capture = |card: AlsaCard| {
            let mut active_capture = None;
            let hint_mutex_capture = hint_mutex.clone();
            let mut cooloff = false;
            let mut cooloff_start = Instant::now();

            Box::new(Capture::new(Box::new(move |output| {
                output.active = false;
                if active_capture.is_none() {
                    // let now = Instant::now();
                    // let hint = HintIter::new(None, &*CString::new("pcm").unwrap()).unwrap().find(|x| {
                    //     if let Some(ref name) = x.name {
                    //         name == card.pcm_hint
                    //     }
                    //     else {
                    //         false
                    //     }
                    // });
                    if cooloff {
                        if Instant::now().duration_since(cooloff_start).as_secs() > 0 {
                            cooloff = false;
                        }
                        else {
                            return;
                        }
                    }
                    let hint = if let Ok(mut hints) = hint_mutex_capture.try_lock() {
                        if let Some(hint) = (*hints).iter().find(|hint| {
                            **hint == card.pcm_hint
                        }).map(|x| x.clone()) {
                            for _ in 0..hints.len() {
                                (*hints).pop().unwrap();
                            }
                            Some(hint)
                        }
                        else {
                            None
                        }
                    }
                    else {
                        None
                    };
                    // println!("search {:?} {:?}", card.pcm_name, Instant::now().duration_since(now));

                    let mut maybe_pcm = if hint.is_some() {
                        if let Ok(pcm) = PCM::open(&*CString::new(card.pcm_name).unwrap(), Direction::Capture, true) {
                            card.hw_params.set_params(&pcm);
                            card.sw_params.set_params(&pcm);

                            println!("connect {:?} capture", card.pcm_name);
                            Some(pcm)
                        }
                        else {
                            None
                        }
                    }
                    else {
                        None
                    };

                    if let Some(pcm) = maybe_pcm.take() {
                        let mut buffer = Vec::new();
                        let mut reading = false;

                        output.clear();
                        let pcm_name = card.pcm_name.clone();
                        let start_threshold = card.sw_params.start_threshold as usize;

                        active_capture = Some(Box::new(move |output: &mut RingBuffer| {
                            let mut unset = false;
                            let mut cont = if let Ok(status) = pcm.status() {
                                if status.get_state() == State::Disconnected {
                                    println!("disconnected {:?} capture", pcm_name);
                                    unset = true;
                                    false
                                }
                                else if status.get_state() == State::XRun {
                                    println!("overrun {:?} capture", pcm_name);
                                    unset = true;
                                    false
                                }
                                else {
                                    true
                                }
                            }
                            else {
                                println!("error checking status {:?} capture", pcm_name);
                                unset = true;
                                false
                            };
                            let maybe_avail = if cont {
                                if pcm.state() == State::Prepared {
                                    pcm.start().unwrap();
                                }

                                match pcm.avail().map(|x| x as usize) {
                                    Ok(avail) => Some(avail),
                                    Err(_) => {
                                        println!("error checking available {:?} capture", pcm_name);
                                        unset = true;
                                        None
                                    }
                                }
                            }
                            else {
                                None
                            };
                            let maybe_io = if let Some(avail) = maybe_avail {
                                if avail > 0 {
                                    match pcm.io_i16() {
                                        Ok(io) => Some(io),
                                        Err(_) => {
                                            println!("error creating io {:?} capture", pcm_name);
                                            unset = true;
                                            None
                                        }
                                    }
                                }
                                else {
                                    None
                                }
                            }
                            else {
                                None
                            };
                            let maybe_read = if let (Some(avail), Some(io)) = (maybe_avail, maybe_io) {
                                if !reading && avail >= start_threshold {
                                    reading = true;
                                }
                                if reading {
                                    for _ in buffer.len()..(avail * 2) {
                                        buffer.push(0 as i16);
                                    }
                                    match io.readi(&mut buffer[..(avail * 2)]) {
                                        Ok(read) => Some(read),
                                        Err(_) => {
                                            println!("error reading {:?} capture", pcm_name);
                                            unset = true;
                                            None
                                        }
                                    }
                                }
                                else {
                                    None
                                }
                            }
                            else {
                                None
                            };
                            if let Some(read) = maybe_read {
                                output.write_from(read * 2, &buffer);
                            }
                            unset
                        }))
                    }
                }
                else {
                    let unset = if let Some(ref mut capture) = active_capture {
                        output.active = true;
                        capture(output)
                    }
                    else {
                        false
                    };
                    if unset {
                        active_capture = None;
                        cooloff = true;
                        cooloff_start = Instant::now();
                    }
                }
            })))
        };

        let duck = |peak| {
            let mut buffer = Vec::new();
            let mut active = false;
            let mut last_peak = Instant::now();
            Box::new(Callback::new(Box::new(move |input, output| {
                let avail = input.len();
                let amount = input.read_into(avail, &mut buffer);
                for i in 0..amount {
                    if buffer[i] > peak {
                        active = true;
                        last_peak = Instant::now();
                    }
                }
                if active {
                    output.write_from(amount, &buffer);
                    if Instant::now().duration_since(last_peak).as_secs() > 1 {
                        active = false;
                    }
                }
                else {
                    for i in 0..amount {
                        buffer[i] = 0;
                    }
                    output.write_from(amount, &buffer);
                }
            })))
        };

        let volume = |(num, denom): (i16, i16)| {
            let mut buffer = Vec::new();
            Box::new(Callback::new(Box::new(move |input, output| {
                let avail = input.len();
                let amount = input.read_into(avail, &mut buffer);
                for i in 0..amount {
                    buffer[i] = buffer[i] * num / denom;
                }
                output.write_from(amount, &buffer);
            })))
        };

        let toslink_out_id = graph.connect(alsa_playback(AlsaCard {
            pcm_name: "toslink16",
            // pcm_hint: "default:CARD=USBStreamer",
            pcm_hint: "USBStreamer",
            hw_params: AlsaHwParams {
                period_size: 96,
                periods: 32,
                ..Default::default()
            },
            sw_params: AlsaSwParams {
                avail_min: 96 * 4,
                start_threshold: 96 * 4,
            },
        }), GraphNodeParams { ..Default::default() });

        let toslink_in_id = graph.connect(alsa_capture(AlsaCard {
            pcm_name: "toslink16",
            // pcm_hint: "default:CARD=USBStreamer",
            pcm_hint: "USBStreamer",
            hw_params: AlsaHwParams {
                period_size: 48,
                periods: 64,
                ..Default::default()
            },
            sw_params: AlsaSwParams {
                avail_min: 96 * 4,
                start_threshold: 96 * 4,
            },
            ..Default::default()
        }), GraphNodeParams {
            to: vec!(toslink_out_id),
            ..Default::default()
        });

        let device_out_id = graph.connect(alsa_playback(AlsaCard {
            pcm_name: "front:CARD=Device,DEV=0",
            pcm_hint: "USB Sound Device",
            hw_params: AlsaHwParams {
                period_size: 96,
                periods: 32,
                ..Default::default()
            },
            // sw_params: AlsaSwParams {
            //     avail_min: 96 * 2,
            //     start_threshold: 96 * 2,
            // },
            ..Default::default()
        }), GraphNodeParams { ..Default::default() });

        // let device_out_id = graph.connect(alsa_playback(AlsaCard {
        //     pcm_name: "ps4",
        //     pcm_hint: "USB Audio Device",
        //     hw_params: AlsaHwParams {
        //         period_size: 768,
        //         periods: 32,
        //         ..Default::default()
        //     },
        //     // sw_params: AlsaSwParams {
        //     //     avail_min: 96 * 2,
        //     //     start_threshold: 96 * 2,
        //     // },
        //     ..Default::default()
        // }), GraphNodeParams { ..Default::default() });

        let transmitter_out_id = graph.connect(alsa_playback(AlsaCard {
            pcm_name: "front:CARD=Transmitter,DEV=0",
            // pcm_hint: "default:CARD=Transmitter",
            pcm_hint: "ASTRO Wireless Transmitter",
            hw_params: AlsaHwParams {
                period_size: 96,
                periods: 32,
                ..Default::default()
            },
            sw_params: AlsaSwParams {
                avail_min: 96 * 2,
                start_threshold: 96 * 2,
            },
            ..Default::default()
        }), GraphNodeParams { ..Default::default() });

        let device_duck_in_id = graph.connect(duck(2000), GraphNodeParams {
            to: vec!(transmitter_out_id),
            ..Default::default()
        });

        let device_in_id = graph.connect(alsa_capture(AlsaCard {
            pcm_name: "front:CARD=Device,DEV=0",
            pcm_hint: "USB Sound Device",
            hw_params: AlsaHwParams {
                period_size: 96,
                periods: 32,
                ..Default::default()
            },
            sw_params: AlsaSwParams {
                avail_min: 96 * 2,
                start_threshold: 96 * 2,
            },
            ..Default::default()
        }), GraphNodeParams {
            to: vec!(device_duck_in_id),
            ..Default::default()
        });

        // let device_in_id = graph.connect(alsa_capture(AlsaCard {
        //     pcm_name: "ps4stereo",
        //     pcm_hint: "USB Audio Device",
        //     hw_params: AlsaHwParams {
        //         period_size: 768,
        //         periods: 8,
        //         ..Default::default()
        //     },
        //     // sw_params: AlsaSwParams {
        //     //     avail_min: 96 * 2,
        //     //     start_threshold: 96 * 2,
        //     // },
        //     ..Default::default()
        // }), GraphNodeParams {
        //     to: vec!(device_duck_in_id),
        //     ..Default::default()
        // });

        let mic_in_duck = graph.connect(duck(6500), GraphNodeParams {
            to: vec!(transmitter_out_id),
            ..Default::default()
        });

        // let mut silence_buffer = Vec::new();
        // let mut out_buffer = Vec::new();
        // let mic_in_duck = graph.connect(Box::new(Callback::new(Box::new(move |input, output| {
        //     let avail = input.len();
        //     input.read_into(avail, &mut silence_buffer);
        //     for _ in out_buffer.len()..avail {
        //         out_buffer.push(0);
        //     }
        //     for i in 0..avail {
        //         silence_buffer[i] = 0;
        //         out_buffer[i] = 0;
        //     }
        //     output.write_from(avail, &out_buffer);
        //     // print!("{:?} ", output.buffer.as_ptr());
        // }))), GraphNodeParams {
        //     to: vec!(transmitter_out_id, device_out_id),
        //     ..Default::default()
        // });

        // let streammic_volume_id = graph.connect(volume((2, 1)), GraphNodeParams {
        //    to: vec!(mic_in_duck),
        //    ..Default::default()
        // });

        let streammic_in_id = graph.connect(alsa_capture(AlsaCard {
            // pcm_name: "streammic",
            pcm_name: "front:CARD=On,DEV=0",
            // pcm_hint: "default:CARD=On",
            pcm_hint: "Turtle Beach Stream Mic (Mic On",
            hw_params: AlsaHwParams {
                period_size: 48,
                periods: 64,
                ..Default::default()
            },
            // sw_params: AlsaSwParams {
            //     avail_min: 96 * 2,
            //     start_threshold: 96 * 2,
            // },
            ..Default::default()
        }), GraphNodeParams {
            // to: vec!(streammic_volume_id),
            to: vec!(mic_in_duck),
            // to: vec!(transmitter_out_id),
            ..Default::default()
        });

        let transmitter_in_id = graph.connect(alsa_capture(AlsaCard {
            pcm_name: "transmitter_stereo",
            // pcm_hint: "default:CARD=Transmitter",
            pcm_hint: "ASTRO Wireless Transmitter",
            hw_params: AlsaHwParams {
                period_size: 96,
                periods: 32,
                ..Default::default()
            },
            sw_params: AlsaSwParams {
                avail_min: 96 * 2,
                start_threshold: 96 * 2,
            },
            ..Default::default()
        }), GraphNodeParams {
            to: vec!(mic_in_duck),
            ..Default::default()
        });

        let sound_buffer = |start : usize, start_amount : usize, stop : usize, overrun : usize| {
            let mut music_buffer = RingBuffer::new();
            music_buffer.max_length = start * 2 * 2;
            let mut start_reached = false;
            let mut music_last = Instant::now();
            let mut music_disabled = false;
            let mut sample_error = 0;
            let mut samples_out = 0;

            Box::new(Callback::new(Box::new(move |input, output| {
                output.active = false;
                if music_disabled && Instant::now().duration_since(music_last).subsec_nanos() > 50000000 {
                    music_disabled = false;
                }
                if !music_disabled {
                    music_buffer.write_from_ring(input.len(), input);
                }
                if !music_disabled && !start_reached && music_buffer.len() > start * 2 {
                    start_reached = true;
                    music_last = Instant::now();
                    output.write_from_ring(min(music_buffer.len(), start_amount * 2 as usize), &mut music_buffer);
                }
                if start_reached {
                    output.active = true;
                    let since_last = Instant::now();
                    let since = since_last.duration_since(music_last);
                    let samples = (((since.subsec_nanos() + sample_error as u32) as f64 / 1000000.0).floor() as usize) * 48;
                    sample_error = (since.subsec_nanos() as usize + sample_error - samples / 48 * 1000000) as usize;
                    // let samples = max(start_amount - output.len() / 2, 0);
                    // print!("samples {:?} {:?} {:?}", music_buffer.len(), start_amount, output.len());
                    if samples < overrun {
                        output.write_from_ring(min(music_buffer.len(), samples * 2), &mut music_buffer);
                        music_last = since_last;
                    }
                    else {
                        print!("music overrun");
                        output.clear();
                        music_buffer.clear();
                        music_disabled = true;
                        music_last = Instant::now();
                        sample_error = 0;
                    }
                    if music_buffer.len() < stop * 2 {
                        print!("empty music buffer");
                        start_reached = false;
                        sample_error = 0;
                    }
                }
                // print!("{:?} ", output.buffer.as_ptr());
            })))
        };

        // let music_out_id = graph.connect(alsa_playback(AlsaCard {
        //     pcm_name: "transmitter",
        //     // pcm_hint: "default:CARD=Transmitter",
        //     pcm_hint: "ASTRO Wireless Transmitter",
        //     hw_params: AlsaHwParams {
        //         period_size: 48,
        //         periods: 32,
        //         ..Default::default()
        //     },
        //     sw_params: AlsaSwParams {
        //         avail_min: 48 * 2,
        //         start_threshold: 48 * 2,
        //     },
        //     ..Default::default()
        // }), GraphNodeParams { ..Default::default() });

        let music_buffer_id = graph.connect(sound_buffer(24000, 192, 48, 24000), GraphNodeParams {
            to: vec!(transmitter_out_id),
            ..Default::default()
        });

        let http_music_mutex = Arc::new(Mutex::new(RingBuffer::new()));
        let http_music_mutex_clone = http_music_mutex.clone();

        let http_music_in_id = graph.connect(Box::new(Capture::new(Box::new(move |output| {
            if let Ok(mut http_music) = (*http_music_mutex).try_lock() {
                output.write_from_ring((*http_music).len(), &mut *http_music);
            }
        }))), GraphNodeParams {
            to: vec!(music_buffer_id),
            ..Default::default()
        });

        thread::spawn(|| {
            let index = |_: &mut Request| {
                Ok(Response::with((status::Ok, "Hello World!")))
            };

            let music = move |req: &mut Request| {
                println!("music");
                let mut buffer = Vec::<i16>::new();
                for _ in 0..3840 {
                    buffer.push(0);
                }
                let mut inner = RingBuffer::new();
                loop {
                    if let Ok(read) = unsafe {
                        let buffer_ptr = buffer.as_mut_ptr();
                        let mut slice = slice::from_raw_parts_mut(buffer_ptr as *mut u8, 3840 * 2);
                        req.body.read(slice)
                    } {
                        inner.write_from(read / 2, &buffer);
                    }
                    else {
                        if let Ok(mut http_music) = (*http_music_mutex_clone).lock() {
                            (*http_music).clear();
                            // print!("read{:?}", read / 2 / 2);
                        }
                        println!("break music");
                        break;
                    }
                    if inner.len() > 0 {
                        if let Ok(mut http_music) = (*http_music_mutex_clone).try_lock() {
                            if (*http_music).len() < 960 {
                                (*http_music).write_from_ring(inner.len(), &mut inner);
                            }
                            else {
                                inner.clear();
                            }
                            // print!("read{:?}", read / 2 / 2);
                        }
                    }
                    sleep(Duration::from_millis(10));
                    // yield_now();
                }

                Ok(Response::with((status::Ok, "Ok")))
            };

            Iron::new(router!(
                index: get "/" => index,
                music: post "/music/" => music,
            )).http("0.0.0.0:80").unwrap();
        });

        loop {
            // sleep(Duration::from_millis(1));
            yield_now();
            graph.update();
        }
    }
}
