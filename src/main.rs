//! A blinky example for Tessel

// Import the tessel library
extern crate tessel;
// Import the libusb library
extern crate libusb;
// Import the alsa library
extern crate alsa;
// Import the graph_utils library
extern crate graph_utils;
// Import the Tessel API
use tessel::Tessel;
// Import sleep from the standard lib
use std::thread::sleep;
// Import durations from the standard lib
use std::time::Duration;

use std::cmp::min;

use std::ffi::CString;
use alsa::device_name::HintIter;

use alsa::{Direction, ValueOr};
use alsa::pcm::{PCM, HwParams, SwParams, Format, Access, State};

use graph_utils::*;

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
    tessel.led[2].on().unwrap();

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

        let alsa_playback = |card: AlsaCard| {
            let mut maybe_pcm_io = None;
            let mut buffer = Vec::new();
            let pcm_period = card.hw_params.period_size as usize;
            let pcm_max = pcm_period * card.hw_params.periods as usize;

            Box::new(Playback::new(Box::new(move |input| {
                if maybe_pcm_io.is_none() {
                    let hint = HintIter::new(None, &*CString::new("pcm").unwrap()).unwrap().find(|x| {
                        if let Some(ref name) = x.name {
                            name == card.pcm_hint
                        }
                        else {
                            false
                        }
                    });

                    if hint.is_some() {
                        if let Ok(mut pcm) = PCM::open(&*CString::new(card.pcm_name).unwrap(), Direction::Playback, true) {
                            card.hw_params.set_params(&pcm);
                            card.sw_params.set_params(&pcm);

                            println!("connect toslink16 output");

                            maybe_pcm_io = Some(pcm);
                        }
                        else {
                            input.clear();
                        }
                    }
                    else {
                        input.clear();
                    }
                }
                let mut unset = false;
                if let Some(ref mut pcm) = maybe_pcm_io {
                    if let Ok(status) = pcm.status() {
                        if status.get_state() == State::Disconnected || status.get_state() == State::XRun {
                            unset = true;
                        }
                    }
                    else {
                        unset = true;
                    }
                }
                if unset {
                    maybe_pcm_io = None;
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
                                    unset = true;
                                }
                                // print!("p{:?}", avail);
                            }
                            else {
                                unset = true;
                            }
                        }
                        else {
                            // print!("pcm{:?}buffer{:?}", pcm_avail, buffer_avail);
                            // print!("p0");
                        }
                    }
                    else {
                        unset = true;
                    }
                }
                if unset {
                    maybe_pcm_io = None;
                }
            })))
        };

        let alsa_capture = |card: AlsaCard| {
            let mut active_capture = None;

            Box::new(Capture::new(Box::new(move |output| {
                if active_capture.is_none() {
                    let hint = HintIter::new(None, &*CString::new("pcm").unwrap()).unwrap().find(|x| {
                        if let Some(ref name) = x.name {
                            name == card.pcm_hint
                        }
                        else {
                            false
                        }
                    });

                    let mut maybe_pcm = if hint.is_some() {
                        if let Ok(pcm) = PCM::open(&*CString::new(card.pcm_name).unwrap(), Direction::Capture, true) {
                            card.hw_params.set_params(&pcm);
                            card.sw_params.set_params(&pcm);

                            println!("connect toslink16 input");
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

                        active_capture = Some(Box::new(move |output: &mut RingBuffer| {
                            let mut unset = false;
                            let mut cont = if let Ok(status) = pcm.status() {
                                if status.get_state() == State::Disconnected || status.get_state() == State::XRun {
                                    println!("disconnected or overrun");
                                    unset = true;
                                    false
                                }
                                else {
                                    true
                                }
                            }
                            else {
                                println!("error checking status");
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
                                        println!("error checking available");
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
                                            println!("error creating io");
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
                                for _ in buffer.len()..(avail * 2) {
                                    buffer.push(0 as i16);
                                }
                                match io.readi(&mut buffer[..(avail * 2)]) {
                                    Ok(read) => Some(read),
                                    Err(_) => {
                                        println!("error reading");
                                        unset = true;
                                        None
                                    }
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
                        capture(output)
                    }
                    else {
                        false
                    };
                    if unset {
                        active_capture = None;
                    }
                }
            })))
        };

        let toslink_out_id = graph.connect(alsa_playback(AlsaCard {
            pcm_name: "toslink16",
            pcm_hint: "default:CARD=USBStreamer",
            hw_params: AlsaHwParams {
                period_size: 192,
                periods: 8,
                ..Default::default()
            },
            sw_params: AlsaSwParams {
                avail_min: 192 * 4,
                start_threshold: 192 * 4,
            },
        }), GraphNodeParams { ..Default::default() });

        let toslink_in_id = graph.connect(alsa_capture(AlsaCard {
            pcm_name: "toslink16",
            pcm_hint: "default:CARD=USBStreamer",
            hw_params: AlsaHwParams {
                period_size: 48,
                periods: 8,
                ..Default::default()
            },
            ..Default::default()
        }), GraphNodeParams {
            to: vec!(toslink_out_id),
            ..Default::default()
        });

        let transmitter_out_id = graph.connect(alsa_playback(AlsaCard {
            pcm_name: "transmitter",
            pcm_hint: "default:CARD=Transmitter",
            hw_params: AlsaHwParams {
                period_size: 192,
                periods: 8,
                ..Default::default()
            },
            sw_params: AlsaSwParams {
                avail_min: 192 * 4,
                start_threshold: 192 * 4,
            },
        }), GraphNodeParams { ..Default::default() });

        let transmitter_in_id = graph.connect(alsa_capture(AlsaCard {
            pcm_name: "streammic",
            pcm_hint: "default:CARD=On",
            hw_params: AlsaHwParams {
                period_size: 48,
                periods: 8,
                ..Default::default()
            },
            ..Default::default()
        }), GraphNodeParams {
            to: vec!(transmitter_out_id),
            ..Default::default()
        });

        let transmitter_in_id = graph.connect(alsa_capture(AlsaCard {
            pcm_name: "transmitter_stereo",
            pcm_hint: "default:CARD=Transmitter",
            hw_params: AlsaHwParams {
                period_size: 48,
                periods: 8,
                ..Default::default()
            },
            ..Default::default()
        }), GraphNodeParams {
            to: vec!(transmitter_out_id),
            ..Default::default()
        });

        loop {
            sleep(Duration::from_millis(1));
            graph.update();
        }
    }

    {
        // Open default playback device
        let pcm = PCM::open(&*CString::new("front:CARD=Device,DEV=0").unwrap(), Direction::Capture, true).unwrap();
        let pcm2 = PCM::open(&*CString::new("front:CARD=Device,DEV=0").unwrap(), Direction::Playback, true).unwrap();

        // Set hardware parameters: 44100 Hz / Mono / 16 bit
        let hwp = HwParams::any(&pcm).unwrap();
        println!("{:?}", hwp);
        hwp.set_channels(1).unwrap();
        hwp.set_rate(48000, ValueOr::Nearest).unwrap();
        hwp.set_format(Format::s16()).unwrap();
        hwp.set_access(Access::RWInterleaved).unwrap();
        // hwp.set_buffer_size_near(128).unwrap();
        hwp.set_periods(8, ValueOr::Nearest).unwrap();
        hwp.set_period_size_near(96, ValueOr::Nearest).unwrap();
        // hwp.set_buffer_size_near(128).unwrap();
        println!("{:?}", hwp);
        pcm.hw_params(&hwp).unwrap();
        let io = pcm.io_i16().unwrap();

        let hwp2 = HwParams::any(&pcm2).unwrap();
        hwp2.set_channels(2).unwrap();
        hwp2.set_rate(48000, ValueOr::Nearest).unwrap();
        hwp2.set_format(Format::s16()).unwrap();
        hwp2.set_access(Access::RWInterleaved).unwrap();
        hwp2.set_periods(16, ValueOr::Nearest).unwrap();
        hwp2.set_period_size_near(96, ValueOr::Nearest).unwrap();
        // hwp2.set_buffer_size_near(128).unwrap();
        println!("{:?}", hwp2);
        pcm2.hw_params(&hwp2).unwrap();
        let io2 = pcm2.io_i16().unwrap();

        let mut buf = [0i16; 24000];
        println!("avail {:?}", pcm2.avail().unwrap());
        pcm2.prepare().unwrap();
        io2.writei(&buf[..(pcm2.avail().unwrap() as usize * 2)]).unwrap();
        println!("avail {:?}", pcm2.avail().unwrap());
        // pcm2.wait(None).unwrap();
        if (pcm2.state() != State::Running) {
            pcm2.start().unwrap();
        }
        pcm.prepare().unwrap();
        pcm.start().unwrap();
        loop {
            let mut read = 0;
            // println!("avail {:?}", pcm.avail());
            while read < 1536 {
                // sleep(Duration::from_millis(1));
                if pcm.avail().unwrap() == 0 {
                    pcm.wait(Some(1)).unwrap();
                }
                // pcm.avail().unwrap();
                let start = read;
                let mut end = read + pcm.avail().unwrap() as usize;
                if end > (1536 - start) / 2 + start {
                    end = (1536 - start) / 2 + start;
                }
                read += io.readi(&mut buf[start..end]).unwrap() * 2;
                let double_end = end + (end - start);
                for i in (start..double_end).rev().filter(|x| x % 2 == 0) {
                    buf[i] = buf[start + (i - start) / 2];
                    buf[i + 1] = buf[start + (i - start) / 2];
                }
                // println!("avail {:?}", pcm2.avail().unwrap());
                if pcm2.avail().unwrap() == 0 {
                    pcm2.wait(Some(1)).unwrap();
                }
                io2.writei(&buf[start..double_end]).unwrap();
                // println!("written {:?}", io2.writei(&buf[start..end]).unwrap());
                // if (read >  && pcm2.state() != State::Running) {
                //     pcm2.start().unwrap();
                // }
                // println!("read {:?}", end - start);
            }
            // assert_eq!(io.readi(&mut buf[..]).unwrap(), 22050);
            // let mut sum = 0.0;
            // let mut max = 0i32;
            // let mut min = 0i32;
            // for (i, a) in buf.iter().enumerate() {
            //     sum += *a as f32;
            //     if *a as i32 > max {
            //         max = *a as i32;
            //     }
            //     if (*a as i32) < min {
            //         min = *a as i32;
            //     }
            // }
            // println!("avg {:?} max {:?} min {:?}", sum / 1536.0, max, min);
            // io2.writei(&buf[..]).unwrap();
            // if (pcm2.state() != State::Running) {
            //     pcm2.start().unwrap();
            // }
        }
    }

    // Loop forever
    loop {
        // Toggle each LED
        // tessel.led[2].on().unwrap();
        // tessel.led[3].toggle().unwrap();
        // Re-execute the loop after sleeping for 100ms
        // sleep(Duration::from_millis(1));
        // tessel.led[2].off().unwrap();
        // sleep(Duration::from_millis(32));
    }
}
