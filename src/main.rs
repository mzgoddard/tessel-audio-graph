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
use std::cell::*;
use std::slice;
use std::io::{Read, ErrorKind};

use std::net::{TcpListener, TcpStream};

use std::collections::btree_map::BTreeMap;

use std::rc::Rc;
use std::env;
use std::process;
use std::process::{Command};

use std::ffi::CString;
use alsa::device_name::HintIter;
use alsa::card;
use alsa::card::Card;

use alsa::{Direction, ValueOr, Result};
use alsa::ctl::{Ctl, ElemType, ElemValue};
use alsa::hctl::{HCtl, Elem};
use alsa::pcm::{PCM, HwParams, SwParams, Format, Access, State};

use graph_utils::*;

use iron::prelude::*;
use iron::status;
use iron::headers::{Headers, ContentType};
use iron::mime::{Mime, TopLevel, SubLevel};
use iron::request;
use iron::{Timeouts, Protocol};

// use hyper::http::h1::HttpReader;

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
    fn set_params(&self, pcm: &PCM) -> alsa::Result<()> {
        let hwp = match HwParams::any(&pcm) {
            Ok(hwp) => hwp,
            Err(err) => {
                println!("{:?}", err);
                return Err(err);
            },
        };
        if let Err(err) = hwp.set_channels(self.channels) {
            println!("{:?}", err);
            return Err(err);
        }
        if let Err(err) = hwp.set_rate(self.rate, ValueOr::Nearest) {
            println!("{:?}", err);
            return Err(err);
        }
        if let Err(err) = hwp.set_format(self.format) {
            println!("{:?}", err);
            return Err(err);
        }
        if let Err(err) = hwp.set_access(Access::RWInterleaved) {
            println!("{:?}", err);
            return Err(err);
        }
        if let Err(err) = hwp.set_periods(self.periods, ValueOr::Nearest) {
            println!("{:?}", err);
            return Err(err);
        }
        if let Err(err) = hwp.set_period_size_near(self.period_size, ValueOr::Nearest) {
            println!("{:?}", err);
            return Err(err);
        }
        println!("{:?}", hwp);
        if let Err(err) = pcm.hw_params(&hwp) {
            return Err(err);
        }
        Ok(())
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

pub enum HCtlValue {
    Boolean(bool),
    Integer(i32),
    Enumerated(u32),
    Bytes(Vec<u8>),
    Integer64(i64),
}

struct AlsaCard {
    pcm_name: &'static str,
    pcm_hint: &'static str,
    pcm_device: usize,
    hw_params: AlsaHwParams,
    sw_params: AlsaSwParams,
    hctl: BTreeMap<&'static str, Vec<(u32, HCtlValue)>>,
}

impl Default for AlsaCard {
    fn default() -> AlsaCard {
        AlsaCard {
            pcm_name: "default",
            pcm_hint: "default",
            pcm_device: 0,
            hw_params: Default::default(),
            sw_params: Default::default(),
            hctl: BTreeMap::new(),
        }
    }
}

impl AlsaCard {
    fn log_success_control(&self, card_id: &str, name: &str, index: u32) {
        println!("Set {} {} index {}.", card_id, name, index);
    }

    fn log_error_control_not_enough_items(&self, card_id: &str, name: &str, index: u32, count: u32) {
        println!("Can't set {} {} index {}. There are only {} items for this control.", card_id, name, index, count);
    }

    fn log_error_control_set_element(&self, card_id: &str, name: &str, index: u32) {
        println!("Setting {} {} index {} returned nothing.", card_id, name, index);
    }

    fn log_error_control_write(&self, card_id: &str, name: &str) {
        println!("Couldn't write {} {}.", card_id, name);
    }

    fn log_error_control_read(&self, card_id: &str, name: &str) {
        println!("Couldn't read {} {}.", card_id, name);
    }

    fn log_error_control_mismatch(&self, card_id: &str, name: &str, expected: ElemType, actual: ElemType) {
        println!("Can't set {} {}. Control type mismatch. Expected {:?}. Found {:?}.", card_id, name, expected, actual);
    }

    fn configure_control_inner<T>(&self, card_id: &str, name: &str, index: u32, elem: &Elem, expected: ElemType, inner: &Fn(&mut ElemValue) -> Option<T>) {
        if elem.info().unwrap().get_type() == expected {
            if let Ok(mut v) = elem.read() {
                if let Some(_) = inner(&mut v) {
                    if let Ok(true) = elem.write(&v) {
                        self.log_success_control(card_id, name, index);
                    }
                    else {
                        self.log_error_control_write(card_id, name);
                    }
                }
                else {
                    self.log_error_control_set_element(card_id, name, index);
                }
            }
            else {
                self.log_error_control_read(card_id, name);
            }
        }
        else {
            self.log_error_control_mismatch(card_id, name, expected, elem.info().unwrap().get_type());
        }
    }

    // fn configure_control_boolean(&self, card_id: &str, name: &str, index: u32, b: bool) {
    //     self.configure_control_inner(card_id, name, index, ElemType::Boolean, |v| v.set_boolean(index, b));
    // }

    fn configure_control(&self, card_id: &str, elem: Elem, control: (&str, &Vec<(u32, HCtlValue)>)) {
        let (name, values) = control;
        if let Ok(info) = elem.info() {
            for &(index, ref value) in values.iter() {
                if index >= info.get_count() {
                    println!("Can't set {} {} index {}. There are only {} items for this control.", card_id, name, index, info.get_count());
                }
                else {
                    match value {
                        &HCtlValue::Boolean(b) => {
                            self.configure_control_inner(card_id, name, index, &elem, ElemType::Boolean, &|v| v.set_boolean(index, b));
                        },
                        &HCtlValue::Integer(i) => {
                            self.configure_control_inner(card_id, name, index, &elem, ElemType::Integer, &|v| v.set_integer(index, i));
                        },
                        &HCtlValue::Enumerated(e) => {
                            self.configure_control_inner(card_id, name, index, &elem, ElemType::Enumerated, &|v| v.set_enumerated(index, e));
                        },
                        &HCtlValue::Bytes(ref b) => {
                            self.configure_control_inner(card_id, name, index, &elem, ElemType::Bytes, &|v| v.set_bytes(b));
                        },
                        &HCtlValue::Integer64(i) => {
                            self.configure_control_inner(card_id, name, index, &elem, ElemType::Integer64, &|v| v.set_integer64(index, i));
                        },
                    }
                }
            }
        }
    }

    pub fn configure_controls(&self, card_id: &str) {
        if let Ok(mut hctl) = HCtl::open(&*CString::new(format!("hw:{}", card_id)).unwrap(), false) {
            hctl.load().unwrap();
            for elem in hctl.elem_iter() {
                let elem_name = if let Ok(elem_id) = elem.get_id() {
                    if let Ok(elem_name) = elem_id.get_name() {
                        String::from(elem_name)
                    } else {
                        String::from("")
                    }
                }
                else {
                    String::from("")
                };
                for (name, value) in self.hctl.iter() {
                    if elem_name == *name {
                        self.configure_control(card_id, elem, (name, value));
                        break;
                    }
                }
                // println!("{:?} {:?} {:?} {:?} {:?}", card_id, elem.get_id(), elem.info().unwrap().get_type(), elem.info().unwrap().get_count(), elem.read());
            }
        }
    }
}

fn main() {
    println!("{:?}", env::args().collect::<Vec<String>>());
    if env::args().count() == 1 {
        Command::new(env::args().nth(0).unwrap())
        .arg("child")
        .spawn()
        .unwrap()
        .wait()
        .unwrap();
        return;
    }

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

    for t in &["ctl", "rawmidi", "timer", "seq", "hwdep"] {
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
        let should_shutdown_mutex = Arc::new(Mutex::new(false));

        let mut graph = Graph::new();

        let hint_mutex = Arc::new(Mutex::new(Vec::<(String, i32)>::new()));
        let hint_mutex_clone = hint_mutex.clone();
        let hint_condvar = Arc::new(Condvar::new());
        let hint_condvar_clone = hint_condvar.clone();
        let should_shutdown_mutex_clone = should_shutdown_mutex.clone();

        thread::spawn(move || {
            // let mut buffer = Vec::new();
            let mut hints = hint_mutex_clone.lock().unwrap();
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

                {
                    for _ in 0..(*hints).len() {
                        (*hints).pop().unwrap();
                    }
                    for card_result in card::Iter::new() {
                        if let Ok(card) = card_result {
                            if let (Ok(ref longname), index) = (card.get_longname(), card.get_index()) {
                                // (*hints).push((longname.clone(), format!("hw:{},0", index)));
                                (*hints).push((longname.clone(), index));
                            }
                            else {
                                break;
                            }
                        }
                        else {
                            break;
                        }
                        // yield_now();
                    }
                }

                hints = hint_condvar_clone.wait(hints).unwrap();

                if let Ok(should_shutdown) = should_shutdown_mutex_clone.lock() {
                    if *should_shutdown {
                        break;
                    }
                }

                // sleep(Duration::from_millis(1000));
                // {
                //     let start = Instant::now();
                //     loop {
                //         if Instant::now().duration_since(start).as_secs() > 4 {
                //             break;
                //         }
                //         yield_now();
                //     }
                // }
            }
        });

        let activating_device = Arc::new(Mutex::new(-1));
        let next_activating_id = Cell::new(0);

        let alsa_playback = |card: AlsaCard| {
            let should_shutdown_mutex = should_shutdown_mutex.clone();
            let mut maybe_pcm_io = None;
            let mut buffer = Vec::new();
            let pcm_period = card.hw_params.period_size as usize;
            let pcm_max = pcm_period * card.hw_params.periods as usize;
            let hint_mutex_playback = hint_mutex.clone();
            let mut cooloff = false;
            let mut cooloff_start = Instant::now();

            let activating_device_clone = activating_device.clone();
            let activating_id = next_activating_id.get();
            next_activating_id.set(activating_id + 1);
            let mut paused = false;

            Box::new(Playback::new(Box::new(move |input| {
                // if let Ok(should_shutdown) = should_shutdown_mutex.lock() {
                //     if let Some(mut pcm) = maybe_pcm_io {
                //         println!("dropping {:?} playback", card.pcm_name);
                //         if let Err(err) = pcm.drop() {
                //
                //         }
                //     }
                //     return;
                // }
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
                        if Instant::now().duration_since(cooloff_start).as_secs() > 4 {
                            cooloff = false;
                        }
                        else {
                            return;
                        }
                    }

                    let hint = if let Ok(mut hints) = hint_mutex_playback.try_lock() {
                        if let Some((_, name)) = (*hints).iter().find(|hint| {
                            (*hint).0 == card.pcm_hint
                        }).map(|x| x.clone()) {
                            // for _ in 0..hints.len() {
                            //     (*hints).pop().unwrap();
                            // }
                            Some(name)
                        }
                        else {
                            None
                        }
                    }
                    else {
                        None
                    };

                    if hint.is_some() {
                        if let Ok(mut activating) = activating_device_clone.lock() {
                            if *activating == -1 {
                                *activating = activating_id;
                                println!("activating {:?} playback", card.pcm_name);
                                return;
                            }
                            else if *activating != activating_id {
                                return;
                            }
                        }
                        else {
                            return;
                        }
                    }

                    if let Some(index) = hint {
                        let card_id = {
                            let card = Card::new(index);
                            if let Ok(mut ctl) = Ctl::from_card(&card, false) {
                                if let Ok(card_info) = ctl.card_info() {
                                    // println!("{:?} {:?} {:?} {:?} {:?} {:?}", card_info.get_id(), card_info.get_driver(), card_info.get_components(), card_info.get_longname(), card_info.get_name(), card_info.get_mixername());
                                    if let Ok(id) = card_info.get_id() {
                                        Some(String::from(id))
                                    }
                                    else {
                                        None
                                    }
                                }
                                else {
                                    None
                                }
                            }
                            else {
                                None
                            }
                        };
                        if let Some(card_id) = card_id {
                            if card.hctl.len() > 0 {
                                card.configure_controls(card_id.as_str());
                            }
                            if let Ok(mut pcm) = PCM::open(&*CString::new(format!("hw:{},0", card_id)).unwrap(), Direction::Playback, true) {
                                if let Err(_) = card.hw_params.set_params(&pcm) {
                                    println!("error setting hwparams {:?} playback", card.pcm_name);
                                    if let Ok(mut activating) = activating_device_clone.lock() {
                                        if *activating == activating_id {
                                            *activating = -1;
                                            println!("failed to activate {:?} playback", card.pcm_name);
                                        }
                                    }
                                    cooloff = true;
                                    cooloff_start = Instant::now();
                                }
                                card.sw_params.set_params(&pcm);

                                println!("connect {:?} playback", card.pcm_name);

                                if let Ok(mut activating) = activating_device_clone.lock() {
                                    if *activating == activating_id {
                                        *activating = -1;
                                        println!("activated {:?} playback", card.pcm_name);
                                    }
                                }

                                input.clear();
                                maybe_pcm_io = Some(pcm);
                            }
                            else {
                                if let Ok(mut activating) = activating_device_clone.lock() {
                                    if *activating == activating_id {
                                        *activating = -1;
                                        println!("failed to activate {:?} playback", card.pcm_name);
                                    }
                                }
                                cooloff = true;
                                cooloff_start = Instant::now();
                            }
                        }
                    }
                    else {
                        cooloff = true;
                        cooloff_start = Instant::now();
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
                            if let Err(_) = pcm.prepare() {
                                println!("error trying to recover {:?} playback", card.pcm_name);
                            }
                            if pcm.state() != State::Prepared {
                                println!("overrun {:?} playback", card.pcm_name);
                                unset = true;
                            }
                        }
                        else if let Ok(mut activating) = activating_device_clone.try_lock() {
                            if *activating == activating_id {
                                *activating = -1;
                            }
                            else if !paused && *activating != -1 {
                                if let Ok(_) = pcm.pause(true) {}
                                paused = true;
                                println!("paused {:?} playback", card.pcm_name);
                            }
                            else if paused && *activating == -1 {
                                if let Ok(_) = pcm.pause(false) {}
                                paused = false;
                                println!("resumed {:?} playback", card.pcm_name);
                                return;
                            }
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
                    if paused {
                        return;
                    }
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
                                    // unset =  true;
                                }
                                // println!("p{:?}", avail);
                            }
                            else {
                                println!("error creating io {:?} playback", card.pcm_name);
                                // unset = true;
                            }
                        }
                        else {
                            // print!("pcm{:?}buffer{:?}", pcm_avail, buffer_avail);
                            // print!("p0");
                        }
                    }
                    else {
                        println!("error checking avail {:?} playback", card.pcm_name);
                        // unset = true;
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

            let activating_device_clone = activating_device.clone();
            let activating_id = next_activating_id.get();
            next_activating_id.set(activating_id + 1);

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
                        if Instant::now().duration_since(cooloff_start).as_secs() > 4 {
                            cooloff = false;
                        }
                        else {
                            return;
                        }
                    }
                    let hint = if let Ok(mut hints) = hint_mutex_capture.try_lock() {
                        if let Some((_, name)) = (*hints).iter().find(|hint| {
                            (*hint).0 == card.pcm_hint
                        }).map(|x| x.clone()) {
                            // for _ in 0..hints.len() {
                            //     (*hints).pop().unwrap();
                            // }
                            Some(name)
                        }
                        else {
                            None
                        }
                    }
                    else {
                        None
                    };
                    if hint.is_some() {
                        if let Ok(mut activating) = activating_device_clone.lock() {
                            if *activating == -1 {
                                *activating = activating_id;
                                println!("activating {:?} capture", card.pcm_name);
                                return;
                            }
                            else if *activating != activating_id {
                                return;
                            }
                        }
                        else {
                            return;
                        }
                    }
                    // println!("search {:?} {:?}", card.pcm_name, Instant::now().duration_since(now));

                    let mut maybe_pcm = if let Some(index) = hint {
                        let card_id = {
                            let card = Card::new(index);
                            if let Ok(mut ctl) = Ctl::from_card(&card, false) {
                                if let Ok(card_info) = ctl.card_info() {
                                    // println!("{:?} {:?} {:?} {:?} {:?} {:?}", card_info.get_id(), card_info.get_driver(), card_info.get_components(), card_info.get_longname(), card_info.get_name(), card_info.get_mixername());
                                    if let Ok(id) = card_info.get_id() {
                                        Some(String::from(id))
                                    }
                                    else {
                                        None
                                    }
                                }
                                else {
                                    None
                                }
                            }
                            else {
                                None
                            }
                        };
                        if let Some(card_id) = card_id {
                            if card.hctl.len() > 0 {
                                card.configure_controls(card_id.as_str());
                            }
                            if let Ok(pcm) = PCM::open(&*CString::new(format!("hw:{},{}", card_id, card.pcm_device)).unwrap(), Direction::Capture, true) {
                                if let Err(_) = card.hw_params.set_params(&pcm) {
                                    println!("error setting hwparams {:?} capture", card.pcm_name);
                                    if let Ok(mut activating) = activating_device_clone.lock() {
                                        if *activating == activating_id {
                                            *activating = -1;
                                            println!("failed to activate {:?} capture", card.pcm_name);
                                        }
                                    }
                                    cooloff = true;
                                    cooloff_start = Instant::now();
                                }
                                card.sw_params.set_params(&pcm);

                                println!("connect {:?} capture", card.pcm_name);

                                if let Ok(mut activating) = activating_device_clone.lock() {
                                    if *activating == activating_id {
                                        *activating = -1;
                                        println!("activated {:?} capture", card.pcm_name);
                                    }
                                }

                                Some(pcm)
                            }
                            else {
                                if let Ok(mut activating) = activating_device_clone.lock() {
                                    if *activating == activating_id {
                                        *activating = -1;
                                        println!("failed to activate {:?} capture", card.pcm_name);
                                    }
                                }
                                cooloff = true;
                                cooloff_start = Instant::now();
                                None
                            }
                        }
                        else {
                            None
                        }
                    }
                    else {
                        cooloff = true;
                        cooloff_start = Instant::now();
                        None
                    };

                    if let Some(pcm) = maybe_pcm.take() {
                        let mut buffer = Vec::new();
                        let mut reading = false;

                        output.clear();
                        let pcm_name = card.pcm_name.clone();
                        let num_channels = card.hw_params.channels as usize;
                        let format = card.hw_params.format;
                        let start_threshold = card.sw_params.start_threshold as usize;
                        let activating_device_clone_clone = activating_device_clone.clone();
                        let activating_id_clone = activating_id;
                        let mut paused = false;

                        active_capture = Some(Box::new(move |output: &mut RingBuffer| {
                            let mut unset = false;
                            let mut cont = if let Ok(status) = pcm.status() {
                                if status.get_state() == State::Disconnected {
                                    println!("disconnected {:?} capture", pcm_name);
                                    unset = true;
                                    false
                                }
                                else if status.get_state() == State::XRun {
                                    pcm.prepare().unwrap();
                                    if pcm.state() != State::Prepared {
                                        println!("overrun {:?} capture", pcm_name);
                                        unset = true;
                                        false
                                    }
                                    else {
                                        !paused
                                    }
                                }
                                else if let Ok(mut activating) = activating_device_clone_clone.try_lock() {
                                    if *activating == activating_id_clone {
                                        *activating = -1;
                                    }
                                    else if !paused && *activating != -1 {
                                        if let Ok(_) = pcm.pause(true) {}
                                        paused = true;
                                        println!("paused {:?} capture", pcm_name);
                                    }
                                    else if paused && *activating == -1 {
                                        if let Ok(_) = pcm.pause(false) {}
                                        paused = false;
                                        println!("resumed {:?} capture", pcm_name);
                                        return false;
                                    }
                                    if status.get_state() == State::Prepared {
                                        if let Err(_) = pcm.start() {
                                            println!("error starting {:?} capture", pcm_name);
                                        }
                                    }
                                    !paused
                                }
                                else {
                                    if status.get_state() == State::Prepared {
                                        if let Err(_) = pcm.start() {
                                            println!("error starting {:?} capture", pcm_name);
                                        }
                                    }
                                    !paused
                                }
                            }
                            else {
                                println!("error checking status {:?} capture", pcm_name);
                                unset = true;
                                false
                            };
                            output.active = !paused;
                            let maybe_avail = if cont {
                                match pcm.avail().map(|x| x as usize) {
                                    Ok(avail) => Some(avail),
                                    Err(_) => {
                                        println!("error checking available {:?} capture", pcm_name);
                                        // unset = true;
                                        None
                                    }
                                }
                            }
                            else {
                                None
                            };
                            let maybe_read = if let Some(avail) = maybe_avail {
                                if !reading && avail >= start_threshold {
                                    reading = true;
                                }
                                if reading && avail > 0 {
                                    match format {
                                        Format::S16LE => {
                                            let maybe_io = match pcm.io_i16() {
                                                Ok(io) => Some(io),
                                                Err(_) => {
                                                    println!("error creating io {:?} capture", pcm_name);
                                                    // unset = true;
                                                    None
                                                }
                                            };
                                            if let Some(io) = maybe_io {
                                                for _ in buffer.len()..(avail * num_channels) {
                                                    buffer.push(0 as i16);
                                                }
                                                match io.readi(&mut buffer[..(avail * num_channels)]) {
                                                    Ok(read) => Some(read),
                                                    Err(_) => {
                                                        println!("error reading {:?} capture", pcm_name);
                                                        // unset = true;
                                                        None
                                                    }
                                                }
                                            }
                                            else {
                                                None
                                            }
                                        },
                                        Format::S32LE => {
                                            let maybe_io = match pcm.io_i32() {
                                                Ok(io) => Some(io),
                                                Err(_) => {
                                                    println!("error creating io {:?} capture", pcm_name);
                                                    // unset = true;
                                                    None
                                                }
                                            };
                                            if let Some(io) = maybe_io {
                                                for _ in buffer.len()..(avail * num_channels * 2) {
                                                    buffer.push(0 as i16);
                                                }
                                                unsafe {
                                                    let buffer_ptr = buffer.as_mut_ptr();
                                                    let mut slice = slice::from_raw_parts_mut(buffer_ptr as *mut i32, avail * num_channels);
                                                    match io.readi(&mut slice) {
                                                        Ok(read) => Some(read * 2),
                                                        Err(_) => {
                                                            println!("error reading {:?} capture", pcm_name);
                                                            // unset = true;
                                                            None
                                                        }
                                                    }
                                                }
                                            }
                                            else {
                                                None
                                            }
                                        },
                                        _ => {
                                            println!("unknown format {:?} capture", pcm_name);
                                            None
                                        },
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
                                output.write_from(read * num_channels, &buffer);
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

        let duck = |peak, gate: Rc<Cell<bool>>| {
            // let mut buffer = Vec::new();
            let mut active = false;
            let mut last_peak = Instant::now();
            let mut samples = 0;
            Box::new(Callback::new(Box::new(move |input, output| {
                let avail = input.len();
                let mut slice = input.read_slice(avail);
                samples += slice.len() / 2;
                for i in 0..slice.len() {
                    if slice[i] > peak {
                        active = true;
                        gate.set(true);
                        // last_peak = Instant::now();
                        samples = 0;
                    }
                }
                if !active {
                    for i in 0..slice.len() {
                        slice[i] = 0;
                    }
                }
                output.write_from_read_slice(slice.len(), &slice);
                if active && samples > 48000 {
                    active = false;
                    gate.set(false);
                }
            })))
        };

        let ducked = |gate: Vec<Rc<Cell<bool>>>, volume: (i32, i32)| {
            let mut buffer = Vec::new();
            Box::new(Callback::new(Box::new(move |input, output| {
                if !input.active {return;}
                let avail = input.len();
                input.read_into(avail, &mut buffer);
                if gate.iter().any(|gate| gate.get()) {
                    for i in 0..avail {
                        buffer[i] = (buffer[i] as i32 * volume.0 / volume.1) as i16;
                    }
                }
                output.write_from(avail, &buffer);
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

        let lean = |max_amount| {
            let mut buffer = Vec::new();
            Box::new(Callback::new(Box::new(move |input, output| {
                let mut avail = min(input.len(), max_amount);
                input.read_into(avail, &mut buffer);
                input.clear();
                output.write_from(avail, &buffer);
            })))
        };

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

        let mono_to_stereo = || {
            let mut buffer = Vec::new();
            Box::new(Callback::new(Box::new(move |input, output| {
                let avail = input.len();
                input.read_into(avail, &mut buffer);
                for _ in buffer.len()..avail * 2 {
                    buffer.push(0);
                }
                for i in (0..avail).rev() {
                    buffer[i * 2] = buffer[i];
                    buffer[i * 2 + 1] = buffer[i];
                }
                output.write_from(avail * 2, &buffer);
                // print!("{:?} {:?} {:?} ", avail, (0..2).rev().nth(0), (0..2).rev().last());
            })))
        };

        let gated = |gate_cell: Arc<Mutex<bool>>| {
            Box::new(Callback::new(Box::new(move |input, output| {
                if !input.active {return;}
                if let Ok(gated) = gate_cell.lock() {
                // if !gate_cell.get() {
                    if !*gated {
                        output.active = false;
                        input.clear();
                    }
                    else {
                        output.write_from_ring(input.len(), input);
                    }
                }
            })))
        };

        let switch_gated = |gate_cell: Arc<Mutex<usize>>, active_state: usize| {
            Box::new(Callback::new(Box::new(move |input, output| {
                if !input.active {return;}
                if let Ok(gated) = gate_cell.lock() {
                // if !gate_cell.get() {
                    if *gated != active_state {
                        output.active = false;
                        input.clear();
                    }
                    else {
                        output.write_from_ring(input.len(), input);
                    }
                }
            })))
        };

        let fade_in = || {
            let mut buffer = Vec::new();
            let mut playing = false;
            let mut last = Instant::now();
            let mut volume = 0;
            let mut timeout = 9600;
            let mut samples = 0;
            Box::new(Callback::new(Box::new(move |input, output| {
                let avail = input.len();
                input.read_into(avail, &mut buffer);
                if avail > 0 {
                    // let now = Instant::now();
                    // let since = now.duration_since(last);
                    samples = 0;
                    if !playing {
                        playing = true;
                        volume = 0;
                    }

                    for i in 0..avail {
                        buffer[i] = (buffer[i] as i32 * max(volume, 0) / 48000) as i16;
                        if i % 4 == 0 {
                            volume = min(volume + 8, 48000);
                        }
                    }

                    // last = now;
                }
                else if playing {
                    // let now = Instant::now();
                    // let since = now.duration_since(last);
                    // if since.as_secs() > 0 || since.subsec_nanos() > (timeout * 1e9) as u32 {
                    samples += 48;
                    if samples > timeout {
                        playing = false;
                        volume = 0;
                    }
                }
                if !playing {
                    for i in 0..avail {
                        buffer[i] = 0;
                    }
                }
                output.write_from(avail, &buffer);
            })))
        };

        let dependent_clock = |sampled: Rc<RefCell<usize>>| {
            Box::new(Callback::new(Box::new(move |input, output| {
                let avail = input.len();
                output.write_from_ring(avail, input);
                *sampled.borrow_mut() += avail;
            })))
        };

        let dependency_clock = |sampled: Rc<RefCell<usize>>| {
            Box::new(Callback::new(Box::new(move |input, output| {
                let avail = {
                    let mut sampled = sampled.borrow_mut();
                    let avail = *sampled;
                    *sampled = 0;
                    avail
                };
                output.write_from_ring(avail, input);
            })))
        };

        let toslink_out_id = graph.connect(alsa_playback(AlsaCard {
            pcm_name: "iec958:CARD=Device_1,DEV=0",
            // pcm_hint: "USB Sound Device",
            pcm_hint: "USB Sound Device at usb-101c0000.ehci-1.2, full speed",
            // pcm_name: "toslink16",
            // // pcm_hint: "default:CARD=USBStreamer",
            // pcm_hint: "USBStreamer",
            hw_params: AlsaHwParams {
                period_size: 48,
                periods: 32,
                ..Default::default()
            },
            sw_params: AlsaSwParams {
                avail_min: 48 * 16,
                start_threshold: 48 * 16,
            },
            ..Default::default()
        }), GraphNodeParams { ..Default::default() });

        // let toslink_buffer_id = graph.connect(sound_buffer("toslink", 1536, 768, 0, 1536), GraphNodeParams {
        //     to: vec!(toslink_out_id),
        //     .. Default::default()
        // });

        let toslink_switch_gate = Arc::new(Mutex::new(1));

        let toslink_1_switch_id = graph.connect(switch_gated(toslink_switch_gate.clone(), 1), GraphNodeParams {
            to: vec!(toslink_out_id),
            ..Default::default()
        });

        let toslink_in_id = graph.connect(alsa_capture(AlsaCard {
            pcm_name: "iec958:CARD=Device_1,DEV=0",
            // pcm_hint: "USB Sound Device",
            pcm_hint: "USB Sound Device at usb-101c0000.ehci-1.2, full speed",
            // pcm_name: "toslink16",
            // // pcm_hint: "default:CARD=USBStreamer",
            // pcm_hint: "USBStreamer",
            hw_params: AlsaHwParams {
                period_size: 48,
                periods: 32,
                ..Default::default()
            },
            sw_params: AlsaSwParams {
                avail_min: 48 * 2,
                start_threshold: 48 * 2,
            },
            hctl: vec![
                ("PCM Capture Source", vec!((0, HCtlValue::Enumerated(2)))),
            ].into_iter().collect(),
            ..Default::default()
        }), GraphNodeParams {
            to: vec!(toslink_1_switch_id),
            // to: vec!(toslink_buffer_id),
            ..Default::default()
        });

        let linear_to_pcm = || {
            let mut buffer = Vec::new();
            Box::new(Callback::new(Box::new(move |input, output| {
                let avail = input.len();
                input.read_into(avail, &mut buffer);
                for i in 0..avail / 4 {
                    buffer[i * 2] = buffer[i * 4 + 1];
                    buffer[i * 2 + 1] = buffer[i * 4 + 3];
                }
                output.write_from(avail, &buffer);
            })))
        };

        let last_two_channels = || {
            let mut buffer = Vec::new();
            Box::new(Callback::new(Box::new(move |input, output| {
                let avail = input.len();
                input.read_into(avail, &mut buffer);
                // if avail > 20 {
                //     println!("{:?}", &buffer[0..20]);
                // }
                for i in 0..avail / 20 {
                    buffer[i * 2] = buffer[i * 20 + 17];
                    buffer[i * 2 + 1] = buffer[i * 20 + 19];
                }
                output.write_from(avail / 10, &buffer);
            })))
        };

        // let toslink_2_pcm_id = graph.connect(last_two_channels(), GraphNodeParams {
        //     to: vec!(toslink_out_id),
        //     ..Default::default()
        // });

        // let toslink_2_stereo_id = graph.connect(last_two_channels(), GraphNodeParams {
        //     to: vec!(toslink_2_pcm_id),
        //     ..Default::default()
        // });

        let toslink_2_switch_id = graph.connect(switch_gated(toslink_switch_gate.clone(), 2), GraphNodeParams {
            to: vec!(toslink_out_id),
            ..Default::default()
        });

        // let toslink_2_in_id = graph.connect(alsa_capture(AlsaCard {
        //     pcm_name: "PC toslink",
        //     pcm_device: 1,
        //     pcm_hint: "Creative Technology USB Sound Blaster HD at usb-101c0000.ehci-1.1.4.4.4.4, full",
        //     hw_params: AlsaHwParams {
        //         period_size: 48,
        //         periods: 64,
        //         ..Default::default()
        //     },
        //     sw_params: AlsaSwParams {
        //         avail_min: 48 * 2,
        //         start_threshold: 48 * 2,
        //     },
        //     ..Default::default()
        // }), GraphNodeParams {
        //     to: vec!(toslink_2_switch_id),
        //     ..Default::default()
        // });

        // let toslink_2_in_id = graph.connect(alsa_capture(AlsaCard {
        //     pcm_name: "PC toslink",
        //     // pcm_device: 1,
        //     pcm_hint: "miniDSP USBStreamer at usb-101c0000.ehci-1.1.4.4.4.4, high speed",
        //     hw_params: AlsaHwParams {
        //         channels: 10,
        //         period_size: 48,
        //         format: Format::s32(),
        //         periods: 64,
        //         ..Default::default()
        //     },
        //     sw_params: AlsaSwParams {
        //         avail_min: 48 * 2,
        //         start_threshold: 48 * 2,
        //     },
        //     ..Default::default()
        // }), GraphNodeParams {
        //     to: vec!(toslink_2_switch_id),
        //     ..Default::default()
        // });

        let toslink_2_in_id = graph.connect(alsa_capture(AlsaCard {
            // pcm_name: "front:CARD=Device,DEV=0",
            pcm_name: "PC toslink",
            // pcm_hint: "USB Sound Device",
            pcm_hint: "USB Sound Device at usb-101c0000.ehci-1.1.2.1, full speed",
            // pcm_name: "hd0",
            // pcm_hint: "USB Sound Blaster HD",
            hw_params: AlsaHwParams {
                period_size: 48,
                periods: 32,
                ..Default::default()
            },
            sw_params: AlsaSwParams {
                avail_min: 48 * 2,
                start_threshold: 48 * 2,
            },
            hctl: vec![
                ("PCM Capture Source", vec!((0, HCtlValue::Enumerated(2)))),
            ].into_iter().collect(),
            ..Default::default()
        }), GraphNodeParams {
            // to: vec!(device_duck_in_id),
            to: vec!(toslink_2_switch_id),
            ..Default::default()
        });

        let r48_to_r44 = || {
            let mut silence_buffer = Vec::new();
            let mut out_buffer = Vec::new();
            let mut frames = 9;
            Box::new(Callback::new(Box::new(move |input, output| {
                let mut avail = input.len();

                let mut num = 0;
                let mut denom = 0;
                while num + 96 <= avail {
                    num += 96;
                    denom += 88;
                    frames += 1;
                    if frames == 10 {
                        // avail += 2;
                        denom += 2;
                        frames = 0;
                    }
                }

                input.read_into(num, &mut silence_buffer);

                for _ in out_buffer.len()..denom {
                    out_buffer.push(0);
                }
                for i in 0..denom {
                    out_buffer[i] = silence_buffer[i / 2 * num / denom * 2 + i % 2];
                }

                output.write_from(denom, &out_buffer);
            })))
        };

        let device_out_id = graph.connect(alsa_playback(AlsaCard {
            pcm_name: "PS4 Chat",
            // pcm_hint: "USB Sound Device",
            pcm_hint: "USB Sound Device at usb-101c0000.ehci-1.1.1, full speed",
            // pcm_name: "hd0",
            // pcm_hint: "USB Sound Blaster HD",
            hw_params: AlsaHwParams {
                rate: 44100,
                period_size: 48,
                periods: 64,
                ..Default::default()
            },
            sw_params: AlsaSwParams {
                avail_min: 48 * 32,
                start_threshold: 48 * 32,
            },
            ..Default::default()
        }), GraphNodeParams { ..Default::default() });

        let device_2_out_id = graph.connect(alsa_playback(AlsaCard {
            pcm_name: "PC Chat",
            // pcm_hint: "USB Sound Device",
            pcm_hint: "USB Sound Device at usb-101c0000.ehci-1.1.4.3.1, full speed",
            // pcm_name: "hd0",
            // pcm_hint: "USB Sound Blaster HD",
            hw_params: AlsaHwParams {
                rate: 44100,
                period_size: 48,
                periods: 64,
                ..Default::default()
            },
            sw_params: AlsaSwParams {
                avail_min: 48 * 32,
                start_threshold: 48 * 32,
            },
            ..Default::default()
        }), GraphNodeParams { ..Default::default() });

        let device_48_to_44_id = graph.connect(r48_to_r44(), GraphNodeParams {
            to: vec!(device_out_id, device_2_out_id),
            ..Default::default()
        });

        let device_mix_id = graph.connect(Box::new(BaseMix::new()), GraphNodeParams {
            to: vec!(device_48_to_44_id),
            ..Default::default()
        });

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
            pcm_name: "transmitter",
            // pcm_hint: "default:CARD=Transmitter",
            // pcm_hint: "ASTRO Wireless Transmitter",
            pcm_hint: "Astro Gaming Inc. ASTRO Wireless Transmitter at usb-101c0000.ehci-1.1.4.1, full",
            hw_params: AlsaHwParams {
                period_size: 48,
                periods: 32,
                ..Default::default()
            },
            sw_params: AlsaSwParams {
                avail_min: 48 * 4,
                start_threshold: 48 * 4,
            },
            ..Default::default()
        }), GraphNodeParams { ..Default::default() });

        let transmitter_lean_id = graph.connect(lean(1536), GraphNodeParams {
            to: vec!(transmitter_out_id),
            ..Default::default()
        });

        let office_out_id = graph.connect(alsa_playback(AlsaCard {
            pcm_name: "office",
            pcm_hint: "C-Media Electronics Inc. USB Audio Device at usb-101c0000.ehci-1.1, full speed",
            hw_params: AlsaHwParams {
                rate: 44100,
                period_size: 48,
                periods: 32,
                ..Default::default()
            },
            sw_params: AlsaSwParams {
                avail_min: 48 * 4,
                start_threshold: 48 * 4,
            },
            ..Default::default()
        }), Default::default());

        let office_r48_id = graph.connect(r48_to_r44(), GraphNodeParams {
            to: vec!(office_out_id),
            ..Default::default()
        });

        let transmitter_mix_id = graph.connect(Box::new(BaseMix::new()), GraphNodeParams {
            to: vec!(transmitter_lean_id, office_r48_id),
            // to: vec!(transmitter_out_id),
            ..Default::default()
        });

        // let other_duck_id = graph.connect(duck(2000), GraphNodeParams {
        //     // to: vec!(transmitter_out_id),
        //     to: vec!(transmitter_mix_id),
        //     ..Default::default()
        // });

        let device_duck_gate = Rc::new(Cell::new(false));

        let device_duck_in_id = graph.connect(duck(1000, device_duck_gate.clone()), GraphNodeParams {
            to: vec!(transmitter_mix_id),
            ..Default::default()
        });

        // let device_lean_id = graph.connect(lean(768), GraphNodeParams {
        //     to: vec!(device_duck_in_id),
        //     ..Default::default()
        // });

        // let device_buffer_in_id = graph.connect(sound_buffer("ps4", 6144, 768, 0, 6144), GraphNodeParams {
        //     to: vec!(device_duck_in_id),
        //     ..Default::default()
        // });

        let r44_to_r48 = || {
            let mut silence_buffer = Vec::new();
            let mut out_buffer = Vec::new();
            let mut frames = 0;
            Box::new(Callback::new(Box::new(move |input, output| {
                let mut avail = input.len();

                let mut num = 0;
                let mut denom = 0;
                while num + 90 <= avail {
                    num += 88;
                    denom += 96;
                    frames += 1;
                    if frames == 10 {
                        // avail += 2;
                        num += 2;
                        frames = 0;
                    }
                }

                input.read_into(num, &mut silence_buffer);

                for _ in out_buffer.len()..denom {
                    out_buffer.push(0);
                }
                for i in 0..denom {
                    out_buffer[i] = silence_buffer[i / 2 * num / denom * 2 + i % 2];
                }

                output.write_from(denom, &out_buffer);
            })))
        };

        let device_in_44_to_48 = graph.connect(r44_to_r48(), GraphNodeParams {
            to: vec!(device_duck_in_id),
            // to: vec!(device_buffer_in_id),
            ..Default::default()
        });

        let device_in_id = graph.connect(alsa_capture(AlsaCard {
            // pcm_name: "front:CARD=Device,DEV=0",
            pcm_name: "PS4 Chat",
            // pcm_hint: "USB Sound Device",
            pcm_hint: "USB Sound Device at usb-101c0000.ehci-1.1.1, full speed",
            // pcm_name: "hd0",
            // pcm_hint: "USB Sound Blaster HD",
            hw_params: AlsaHwParams {
                rate: 44100,
                period_size: 48,
                periods: 32,
                ..Default::default()
            },
            sw_params: AlsaSwParams {
                avail_min: 48 * 16,
                start_threshold: 48 * 16,
            },
            hctl: vec![
                ("PCM Capture Source", vec!((0, HCtlValue::Enumerated(0)))),
            ].into_iter().collect(),
            ..Default::default()
        }), GraphNodeParams {
            // to: vec!(device_duck_in_id),
            to: vec!(device_in_44_to_48),
            ..Default::default()
        });

        let device_2_in_44_to_48 = graph.connect(r44_to_r48(), GraphNodeParams {
            to: vec!(device_duck_in_id),
            // to: vec!(device_buffer_in_id),
            ..Default::default()
        });

        // let device_2_stereo_id = graph.connect(mono_to_stereo(), GraphNodeParams {
        //     to: vec!(device_2_in_44_to_48),
        //     ..Default::default()
        // });

        let device_2_in_id = graph.connect(alsa_capture(AlsaCard {
            // pcm_name: "front:CARD=Device,DEV=0",
            pcm_name: "PC Chat",
            // pcm_hint: "USB Sound Device",
            pcm_hint: "USB Sound Device at usb-101c0000.ehci-1.1.4.3.1, full speed",
            // pcm_name: "hd0",
            // pcm_hint: "USB Sound Blaster HD",
            hw_params: AlsaHwParams {
                rate: 44100,
                channels: 2,
                period_size: 48,
                periods: 32,
                ..Default::default()
            },
            sw_params: AlsaSwParams {
                avail_min: 48 * 16,
                start_threshold: 48 * 16,
            },
            hctl: vec![
                // ("Speaker Playback Volume", (vec!((0, HCtlValue::Integer(22)), (1, HCtlValue::Integer(22))))),
                // ("Mic Capture Volume", vec!((0, HCtlValue::Integer(4)))),
                // ("Auto Gain Control", (vec!((0, HCtlValue::Boolean(false))))),
                ("PCM Capture Source", vec!((0, HCtlValue::Enumerated(0)))),
            ].into_iter().collect(),
            ..Default::default()
        }), GraphNodeParams {
            // to: vec!(device_duck_in_id),
            to: vec!(device_2_in_44_to_48),
            ..Default::default()
        });

        // let device_in_id = graph.connect(alsa_capture(AlsaCard {
        //     pcm_name: "front:CARD=Device,DEV=0",
        //     pcm_hint: "USB Sound Device",
        //     hw_params: AlsaHwParams {
        //         period_size: 96,
        //         periods: 32,
        //         ..Default::default()
        //     },
        //     sw_params: AlsaSwParams {
        //         avail_min: 96 * 2,
        //         start_threshold: 96 * 2,
        //     },
        //     ..Default::default()
        // }), GraphNodeParams {
        //     to: vec!(device_duck_in_id),
        //     ..Default::default()
        // });

        let mic_duck_gate = Rc::new(Cell::new(false));

        let mic_in_duck = graph.connect(duck(5500, mic_duck_gate.clone()), GraphNodeParams {
            to: vec!(transmitter_mix_id, device_mix_id),
            ..Default::default()
        });

        let chat_sampled = Rc::new(RefCell::new(0));

        let mic_dependent_clock_id = graph.connect(dependent_clock(chat_sampled.clone()), GraphNodeParams {
            to: vec!(mic_in_duck),
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
            // pcm_hint: "Turtle Beach Stream Mic (Mic On",
            pcm_hint: "Turtle Beach Turtle Beach Stream Mic (Mic On at usb-101c0000.ehci-1.1.4.4.1, fu",
            hw_params: AlsaHwParams {
                period_size: 48,
                periods: 32,
                ..Default::default()
            },
            sw_params: AlsaSwParams {
                avail_min: 48 * 2,
                start_threshold: 48 * 2,
            },
            ..Default::default()
        }), GraphNodeParams {
            // to: vec!(streammic_volume_id),
            to: vec!(mic_dependent_clock_id),
            // to: vec!(transmitter_out_id),
            ..Default::default()
        });

        // let lean_in_id = graph.connect(lean(768), GraphNodeParams {
        //     to: vec!(mic_dependent_clock_id),
        //     ..Default::default()
        // });

        let transmitter_stereo_id = graph.connect(mono_to_stereo(), GraphNodeParams {
            to: vec!(mic_dependent_clock_id),
            ..Default::default()
        });

        let transmitter_in_id = graph.connect(alsa_capture(AlsaCard {
            pcm_name: "transmitter_stereo",
            // pcm_hint: "default:CARD=Transmitter",
            // pcm_hint: "ASTRO Wireless Transmitter",
            pcm_hint: "Astro Gaming Inc. ASTRO Wireless Transmitter at usb-101c0000.ehci-1.1.4.1, full",
            hw_params: AlsaHwParams {
                channels: 1,
                period_size: 48,
                periods: 32,
                ..Default::default()
            },
            sw_params: AlsaSwParams {
                avail_min: 48 * 2,
                start_threshold: 48 * 2,
            },
            ..Default::default()
        }), GraphNodeParams {
            // to: vec!(lean_in_id),
            // to: vec!(mic_in_duck),
            to: vec!(transmitter_stereo_id),
            ..Default::default()
        });

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

        let throw_away_first_4 = || {
            let mut received = 0;
            let mut started = false;
            Box::new(Callback::new(Box::new(move |input, output| {
                let avail = input.len();
                if started {
                    if !input.active {
                        started = false;
                        received = 0;
                    }
                    output.write_from_ring(avail, input);
                }
                else {
                    output.active = false;
                    received += avail;
                    if received > 384000 {
                        started = true;
                    }
                }
            })))
        };

        let content_throw_away_first_id = graph.connect(throw_away_first_4(), GraphNodeParams {
            to: vec!(transmitter_mix_id),
            ..Default::default()
        });

        let content_dependency_clock_id = graph.connect(dependency_clock(chat_sampled.clone()), GraphNodeParams {
            to: vec!(transmitter_mix_id),
            ..Default::default()
        });

        let content_duck_id = graph.connect(ducked(vec!(mic_duck_gate, device_duck_gate), (2, 5)), GraphNodeParams {
            to: vec!(transmitter_mix_id),
            ..Default::default()
        });

        let music_fade_in_id = graph.connect(fade_in(), GraphNodeParams {
            to: vec!(content_duck_id),
            // to: vec!(transmitter_mix_id),
            ..Default::default()
        });

        // let music_buffer_id = graph.connect(sound_buffer("music", 6144, 768, 0, 768), GraphNodeParams {
        //     to: vec!(music_fade_in_id),
        //     // to: vec!(transmitter_out_id),
        //     ..Default::default()
        // });

        let http_music_mutex = Arc::new(Mutex::new(RingBuffer::new()));
        let http_music_mutex_clone = http_music_mutex.clone();
        let tcp_music_mutex_clone = http_music_mutex.clone();
        let net_music_tick_pair = Arc::new((Mutex::new(-1), Condvar::new(), Mutex::new(false), Condvar::new()));
        let net_music_tick_pair_node = net_music_tick_pair.clone();

        let activating_device_clone = activating_device.clone();
        let activating_id = next_activating_id.get();
        next_activating_id.set(activating_id + 1);
        let mut last_received = Instant::now();
        let mut activation_start = Instant::now();
        let mut state = 0;
        let mut paused = false;
        let mut samples = 0;
        let http_music_in_id = graph.connect(Box::new(Capture::new(Box::new(move |output| {
            if let Ok(mut http_music) = (*http_music_mutex).try_lock() {
                // let now = Instant::now();
                output.active = false;
                if state == 0 && (*http_music).len() > 0 {
                    if let Ok(mut activating) = activating_device_clone.lock() {
                        if *activating == -1 {
                            // activation_start = now;
                            *activating = activating_id;
                            println!("activating http music");
                            samples = 0;
                            state = 1;
                        }
                    }
                    // last_received = now;
                    (*http_music).clear();
                }
                // else if state == 1 && (*http_music).len() > 0 && now.duration_since(activation_start).as_secs() > 0 {
                else if state == 1 && (*http_music).len() > 0 && samples > 192000 {
                    if let Ok(mut activating) = activating_device_clone.lock() {
                        if *activating == activating_id {
                            *activating = -1;
                            println!("activated http music");
                            // last_received = now;
                            (*http_music).clear();
                            samples = 0;
                            state = 2;
                        }
                        else {
                            // last_received = now;
                            (*http_music).clear();
                            state = 0;
                        }
                    }
                }
                // else if state == 1 && (*http_music).len() == 0 && now.duration_since(activation_start).as_secs() > 1 {
                else if state == 1 && (*http_music).len() == 0 && samples > 288000 {
                    if let Ok(mut activating) = activating_device_clone.lock() {
                        if *activating == activating_id {
                            *activating = -1;
                            println!("didn't activate http music");
                            state = 0;
                        }
                    }
                }
                else if state == 1 && (*http_music).len() > 0 {
                    samples += (*http_music).len();
                    // last_received = now;
                    (*http_music).clear();
                }
                else if state == 1 {
                    // last_received = now;
                    samples += 20;
                }
                else if state == 2 && (*http_music).len() > 0 {
                    // last_received = now;
                    // samples += (*http_music).len();
                    samples = 0;
                    if let Ok(mut activating) = activating_device_clone.lock() {
                        if *activating != -1 {
                            if !paused {
                                println!("paused http music");
                            }
                            paused = true;
                        }
                        else {
                            if paused {
                                println!("resumed http music");
                            }
                            paused = false;
                        }
                    }
                    if !paused {
                        output.active = true;
                        output.write_from_ring((*http_music).len(), &mut *http_music);
                    }
                    else {
                        (*http_music).clear();
                    }
                }
                // else if state == 2 && (*http_music).len() == 0 && now.duration_since(last_received).as_secs() < 1 {
                else if state == 2 && (*http_music).len() == 0 && samples < 48000 {
                    samples += 20;
                    output.active = !paused;
                }
                // else if state == 2 && (*http_music).len() == 0 && now.duration_since(last_received).as_secs() > 0 {
                else if state == 2 && (*http_music).len() == 0 && samples >= 48000 {
                    state = 0;
                }
                // let was_active = output.active;
                // output.active = (*http_music).len() > 0 || now.duration_since(last_received).subsec_nanos() < 25000000;
                // if !was_active && output.active {
                //     activation_start = now;
                //     if let Ok(mut activating) = activating_device_clone.lock() {
                //         if *activating == -1 {
                //             *activating = activating_id;
                //             println!("activating http music");
                //         }
                //         else if *activating != activating_id {
                //             output.active = false;
                //         }
                //     }
                //     else {
                //         return;
                //     }
                // }
                // else if output.active && now.duration_since(activation_start).as_secs() > 0 {
                //     if let Ok(mut activating) = activating_device_clone.lock() {
                //         if *activating == activating_id {
                //             *activating = -1;
                //             println!("activated http music");
                //         }
                //         else if *activating != -1 {
                //             return;
                //         }
                //         else {
                //             if (*http_music).len() > 0 {
                //                 last_received = now;
                //             }
                //             if output.active {
                //                 output.write_from_ring((*http_music).len(), &mut *http_music);
                //             }
                //             else {
                //                 (*http_music).clear();
                //             }
                //         }
                //     }
                //     else {
                //         return;
                //     }
                // }
                // else if !output.active {
                //     if let Ok(mut activating) = activating_device_clone.lock() {
                //         if *activating == activating_id {
                //             *activating = -1;
                //             println!("didn't activate http music");
                //         }
                //     }
                // }
            }
            {
                // let music_connected = {
                //     let net_guard = net_music_tick_pair_node.0.lock().unwrap();
                //     *net_guard != -1
                // };
                // if music_connected {
                    net_music_tick_pair_node.1.notify_one();
                    yield_now();
                // }
            }
        }))), GraphNodeParams {
            // to: vec!(music_buffer_id),
            // to: vec!(music_fade_in_id),
            // to: vec!(content_duck_id),
            to: vec!(transmitter_mix_id),
            ..Default::default()
        });

        let chrome_fade_in_id = graph.connect(fade_in(), GraphNodeParams {
            to: vec!(content_duck_id),
            // to: vec!(transmitter_mix_id),
            ..Default::default()
        });

        // let chrome_buffer_id = graph.connect(sound_buffer("chrome", 6144, 768, 0, 768), GraphNodeParams {
        //     to: vec!(chrome_fade_in_id),
        //     // to: vec!(transmitter_out_id),
        //     ..Default::default()
        // });

        let chrome_device_gate = Arc::new(Mutex::new(false));
        let chrome_gated_id = graph.connect(gated(chrome_device_gate.clone()), GraphNodeParams {
            to: vec!(device_mix_id),
            ..Default::default()
        });

        let http_chrome_mutex = Arc::new(Mutex::new(RingBuffer::new()));
        let http_chrome_mutex_clone = http_chrome_mutex.clone();
        let tcp_chrome_mutex_clone = http_chrome_mutex.clone();
        let net_chrome_tick_pair = Arc::new((Mutex::new(-1), Condvar::new(), Mutex::new(false), Condvar::new()));
        let net_chrome_tick_pair_node = net_chrome_tick_pair.clone();

        let activating_device_clone = activating_device.clone();
        let activating_id = next_activating_id.get();
        next_activating_id.set(activating_id + 1);
        let mut last_received = Instant::now();
        let mut activation_start = Instant::now();
        let mut state = 0;
        let mut paused = false;
        let mut samples = 0;
        let mut buffer = RingBuffer::new();
        let http_chrome_in_id = graph.connect(Box::new(Capture::new(Box::new(move |output| {
            if let Ok(mut http_chrome) = (*http_chrome_mutex).try_lock() {
                // let now = Instant::now();
                output.active = false;
                if state == 0 && (*http_chrome).len() > 0 {
                    if let Ok(mut activating) = activating_device_clone.lock() {
                        if *activating == -1 {
                            // activation_start = now;
                            *activating = activating_id;
                            println!("activating http chrome");
                            buffer.clear();
                            samples = 0;
                            state = 1;
                        }
                    }
                    // last_received = now;
                    (*http_chrome).clear();
                }
                // else if state == 1 && (*http_chrome).len() > 0 && now.duration_since(activation_start).as_secs() > 0 {
                else if state == 1 && (*http_chrome).len() > 0 && samples > 48000 {
                    if let Ok(mut activating) = activating_device_clone.lock() {
                        if *activating == activating_id {
                            *activating = -1;
                            println!("activated http chrome");
                            // last_received = now;
                            // (*http_chrome).clear();
                            buffer.write_from_ring((*http_chrome).len(), &mut (*http_chrome));
                            let mut avail = buffer.len();
                            buffer.read_slice(avail - max(avail as i32 - 768, 0) as usize);
                            output.active = true;
                            avail = buffer.len();
                            output.write_from_ring(avail, &mut buffer);
                            samples = 0;
                            state = 2;
                        }
                        else {
                            // last_received = now;
                            (*http_chrome).clear();
                            state = 0;
                        }
                    }
                }
                // else if state == 1 && (*http_chrome).len() == 0 && now.duration_since(activation_start).as_secs() > 1 {
                else if state == 1 && (*http_chrome).len() == 0 && samples > 96000 {
                    if let Ok(mut activating) = activating_device_clone.lock() {
                        if *activating == activating_id {
                            *activating = -1;
                            println!("didn't activate http chrome");
                            state = 0;
                        }
                    }
                }
                else if state == 1 && (*http_chrome).len() > 0 {
                    // last_received = now;
                    samples += (*http_chrome).len();
                    buffer.write_from_ring((*http_chrome).len(), &mut (*http_chrome));
                    // (*http_chrome).clear();
                }
                else if state == 1 {
                    // last_received = now;
                    samples += 20;
                }
                else if state == 2 && (*http_chrome).len() > 0 {
                    // last_received = now;
                    samples = 0;
                    if let Ok(mut activating) = activating_device_clone.lock() {
                        if *activating != -1 {
                            if !paused {
                                println!("paused http chrome");
                            }
                            paused = true;
                        }
                        else {
                            if paused {
                                println!("resumed http chrome");
                            }
                            paused = false;
                        }
                    }
                    if !paused {
                        output.active = true;
                        output.write_from_ring((*http_chrome).len(), &mut *http_chrome);
                    }
                    else {
                        (*http_chrome).clear();
                    }
                }
                // else if state == 2 && (*http_chrome).len() == 0 && now.duration_since(last_received).as_secs() < 1 {
                else if state == 2 && (*http_chrome).len() == 0 && samples < 48000 {
                    samples += 20;
                    output.active = !paused;
                }
                // else if state == 2 && (*http_chrome).len() == 0 && now.duration_since(last_received).as_secs() > 0 {
                else if state == 2 && (*http_chrome).len() == 0 && samples >= 48000 {
                    println!("deactivated http chrome");
                    state = 0;
                }
            }
            net_chrome_tick_pair_node.1.notify_one();
            // if let Ok(mut http_chrome) = (*http_chrome_mutex).try_lock() {
            //     let now = Instant::now();
            //     output.active = now.duration_since(last_received).subsec_nanos() < 25000000;
            //     if (*http_chrome).len() > 0 {
            //         last_received = now;
            //     }
            //     output.write_from_ring((*http_chrome).len(), &mut *http_chrome);
            // }
        }))), GraphNodeParams {
            // to: vec!(music_buffer_id),
            // to: vec!(chrome_buffer_id, chrome_gated_id),
            to: vec!(chrome_fade_in_id, chrome_gated_id),
            // to: vec!(transmitter_mix_id, device_out_id),
            ..Default::default()
        });

        let audio_stream_factory = move |name, stream_mutex: Arc<Mutex<RingBuffer>>, should_shutdown_mutex: Arc<Mutex<bool>>, net_tick_pair: Arc<(Mutex<i32>, Condvar, Mutex<bool>, Condvar)>| {
            move |stream: &mut Read| {
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
                //     net_out_condvar.notify_one();
                // }
                net_guard = net_condvar.wait(net_guard).unwrap();

                // let mut total_read = 0;
                // let start = Instant::now();
                // loop {
                //     if let Ok(read) = unsafe {
                //         let buffer_ptr = buffer.as_mut_ptr();
                //         let mut slice = slice::from_raw_parts_mut(buffer_ptr as *mut u8, 96 * 2 * 2);
                //         stream.read(slice)
                //     } {
                //         if read == 0 {
                //             break;
                //         }
                //         total_read += read;
                //         if Instant::now().duration_since(start).subsec_nanos() > 10000000 {
                //             break;
                //         }
                //     }
                //     else {
                //         break;
                //     }
                // }
                // println!("{} read {} {} {}", name, total_read, Instant::now().duration_since(start).as_secs(), Instant::now().duration_since(start).subsec_nanos());

                let mut last_read = Instant::now();
                let mut music_last = Instant::now();
                let mut samples_received = 0u64;
                loop {
                    if let Ok(should_shutdown) = should_shutdown_mutex.lock() {
                        if *should_shutdown {
                            break;
                        }
                    }
                    // let start = Instant::now();
                    // let since_last = Instant::now();
                    // let since = since_last.duration_since(music_last);
                    // let should_received = since.as_secs() * 48000 + since.subsec_nanos() as u64 / 1000000 * 48;
                    // let mut samples = should_received - samples_received;
                    // samples = min(samples, 1536);
                    // samples_received += samples;
                    // music_last = since_last;
                    // let ms = (since.subsec_nanos() + sample_error as u32) / 1000000;
                    // let mut samples = ms as usize * 48;
                    // samples = min(samples, 1536);
                    // // let samples = 192;
                    // let ns = samples / 48 * 1000000;
                    // sample_error = sample_error + since.subsec_nanos() as usize - ns as usize;
                    // music_last = since_last;
                    // print!("{} ", samples);
                    // if samples > 0 {
                    let start = Instant::now();
                    let mut did_error = false;
                    let since_last = Instant::now();
                    let since = since_last.duration_since(music_last);
                    let should_received = since.as_secs() * 48000 + since.subsec_nanos() as u64 / 1000000 * 48;
                    loop {
                        // let mut samples = should_received - samples_received;
                        // samples = min(samples, 192);
                        // samples_received += samples;

                        let samples = 192;
                        match unsafe {
                            let buffer_ptr = buffer.as_mut_ptr();
                            let mut slice = slice::from_raw_parts_mut(buffer_ptr as *mut u8, samples as usize * 2 * 2);
                            stream.read(slice)
                        } {
                            Ok(read) => {
                                // if read == 0 {
                                //     break;
                                // }
                                // unsafe {
                                //     let buffer_ptr = buffer.as_mut_ptr();
                                //     let mut slice_read = slice::from_raw_parts_mut(buffer_ptr as *mut i8, samples * 2);
                                //     let mut slice_write = slice::from_raw_parts_mut(buffer_ptr as *mut i16, samples * 2);
                                //     for i in (0..samples).rev() {
                                //         slice_write[i * 2] = (slice_read[i * 2]) as i16;
                                //         slice_write[i * 2 + 1] = (slice_read[i * 2 + 1]) as i16;
                                //     }
                                // }
                                // samples_received -= samples - read as u64 / 2 / 2;
                                samples_received += read as u64 / 2 / 2;
                                // sample_error += (samples - read / 2 / 2) * 1000000 / 48;
                                // print!("{} {} ", name, read);
                                inner.write_from(read / 2, &buffer);
                                if Instant::now().duration_since(start).subsec_nanos() > 1500000 {
                                    // print!("read for 2ms ");
                                    break;
                                }
                                if samples_received > should_received {
                                    // print!("read enough samples ");
                                    break;
                                }
                                // if read / 2 / 2 < samples as usize {
                                if read == 0 {
                                    // print!("less than expected {} {} ", read / 2 / 2, samples);
                                    break;
                                }
                            },
                            Err(err) => {
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
                        if let Ok(mut http_music) = (*stream_mutex).try_lock() {
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
                        }
                    }
                    // print!("{} {:?} ", name, Instant::now().duration_since(start));
                    // sleep(Duration::from_millis(1));
                    // {
                    //     let mut net_out_guard = net_out_mutex.lock().unwrap();
                    //     // *net_out_guard = true;
                    //     net_out_condvar.notify_one();
                    // }
                    // yield_now();
                    net_guard = net_condvar.wait(net_guard).unwrap();
                    // {
                    //     let start = Instant::now();
                    //     while Instant::now().duration_since(start).subsec_nanos() < 1000000 {
                    //         yield_now();
                    //     }
                    // }
                }
                // {
                //     let mut net_out_guard = net_out_mutex.lock().unwrap();
                //     *net_out_guard = false;
                //     net_out_condvar.notify_one();
                // }
                *net_guard = -1;
            }
        };

        let http_audio_stream_factory = move |name, stream_mutex, should_shutdown_mutex, net_tick_pair| {
            let cb = audio_stream_factory(name, stream_mutex, should_shutdown_mutex, net_tick_pair);
            move |req: &mut Request| {
                cb(&mut req.body);

                Ok(Response::with((status::Ok, "Ok")))
            }
        };

        let audio_stream_factory = move |name, stream_mutex: Arc<Mutex<RingBuffer>>, should_shutdown_mutex: Arc<Mutex<bool>>, net_tick_pair: Arc<(Mutex<i32>, Condvar, Mutex<bool>, Condvar)>| {
            move |stream: &mut Read| {
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
                net_guard = net_condvar.wait(net_guard).unwrap();

                loop {
                    let samples = 192;
                    match unsafe {
                        let buffer_ptr = buffer.as_mut_ptr();
                        let mut slice = slice::from_raw_parts_mut(buffer_ptr as *mut u8, samples as usize * 2 * 2);
                        stream.read(slice)
                    } {
                        Ok(read) => {
                            // samples_received += read as u64 / 2 / 2;
                            // inner.write_from(read / 2, &buffer);
                            // if Instant::now().duration_since(start).subsec_nanos() > 500000 {
                            //     // print!("read for 2ms ");
                            //     break;
                            // }
                            // if samples_received > should_received {
                            //     // print!("read enough samples ");
                            //     break;
                            // }
                            // // if read / 2 / 2 < samples as usize {
                            // if read == 0 {
                            //     // print!("read nothing ");
                            //     break;
                            // }
                        },
                        Err(err) => {
                            if err.kind() == ErrorKind::WouldBlock {
                                // print!("would block {} ", Instant::now().duration_since(start).subsec_nanos());
                                break;
                            }
                            else {
                                // did_error = true;
                                if let Ok(mut http_music) = (*stream_mutex).lock() {
                                    (*http_music).clear();
                                    // print!("read{:?}", read / 2 / 2);
                                }
                                println!("break {:?} error: {:?}", name, err);
                                break;
                            }
                        },
                    }
                }

                let mut last_read = Instant::now();
                let mut music_last = Instant::now();
                let mut samples_received = 0u64;
                loop {
                    if let Ok(should_shutdown) = should_shutdown_mutex.lock() {
                        if *should_shutdown {
                            break;
                        }
                    }

                    let start = Instant::now();
                    let mut did_error = false;
                    let since_last = Instant::now();
                    let since = since_last.duration_since(music_last);
                    let should_received = since.as_secs() * 48000 + since.subsec_nanos() as u64 / 1000000 * 48;
                    loop {
                        let samples = 192;
                        match unsafe {
                            let buffer_ptr = buffer.as_mut_ptr();
                            let mut slice = slice::from_raw_parts_mut(buffer_ptr as *mut u8, samples as usize * 2 * 2);
                            stream.read(slice)
                        } {
                            Ok(read) => {
                                samples_received += read as u64 / 2 / 2;
                                inner.write_from(read / 2, &buffer);
                                if Instant::now().duration_since(start).subsec_nanos() > 500000 {
                                    // print!("read for 2ms ");
                                    break;
                                }
                                // if samples_received > should_received {
                                //     // print!("read enough samples ");
                                //     break;
                                // }
                                // // if read / 2 / 2 < samples as usize {
                                if read == 0 {
                                    // print!("read nothing ");
                                    break;
                                }
                            },
                            Err(err) => {
                                if err.kind() == ErrorKind::WouldBlock {
                                    // print!("would block {} ", Instant::now().duration_since(start).subsec_nanos());
                                    break;
                                }
                                else {
                                    did_error = true;
                                    if let Ok(mut http_music) = (*stream_mutex).lock() {
                                        (*http_music).clear();
                                        // print!("read{:?}", read / 2 / 2);
                                    }
                                    println!("break {:?} error: {:?}", name, err);
                                    break;
                                }
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
                        if let Ok(mut http_music) = (*stream_mutex).try_lock() {
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
                        }
                    }
                    net_guard = net_condvar.wait(net_guard).unwrap();
                    // yield_now();
                }
                *net_guard = -1;
            }
        };

        let tcp_audio_stream_factory = move |name, stream_mutex: Arc<Mutex<RingBuffer>>, should_shutdown_mutex: Arc<Mutex<bool>>, net_tick_pair: Arc<(Mutex<i32>, Condvar, Mutex<bool>, Condvar)>| {
            // let cb = audio_stream_factory(name, stream_mutex);
            let stream_mutex = stream_mutex.clone();
            let should_shutdown_mutex = should_shutdown_mutex.clone();
            let net_tick_pair = net_tick_pair.clone();
            let cb = audio_stream_factory(name, stream_mutex, should_shutdown_mutex, net_tick_pair);
            move |mut stream: TcpStream| {
                // stream.set_read_timeout(Some(Duration::from_millis(1))).unwrap();
                stream.set_nonblocking(true).unwrap();
                cb(&mut stream);
            }
        };

        let should_shutdown_mutex_http = should_shutdown_mutex.clone();
        let toslink_switch_gate_http = toslink_switch_gate.clone();
        let chrome_device_gate_http = chrome_device_gate.clone();

        let net_start_pair = Arc::new((Mutex::new(false), Condvar::new()));
        let net_start_pair_clone = net_start_pair.clone();

        let net_music_tick_pair_clone = net_music_tick_pair.clone();
        let net_chrome_tick_pair_clone = net_chrome_tick_pair.clone();

        thread::spawn(move || {
            // {
            //     let start = Instant::now();
            //     loop {
            //         if Instant::now().duration_since(start).as_secs() > 4 {
            //             break;
            //         }
            //         yield_now();
            //     }
            // }

            {
                let &(ref net_start, ref condvar) = &*net_start_pair_clone;
                let start = net_start.lock().unwrap();
                condvar.wait(start).unwrap();
            }

            println!("starting http");

            let renderFactory = || {
                let toslink_switch_gate_render = toslink_switch_gate_http.clone();
                let chrome_device_gate_render = chrome_device_gate_http.clone();
                move || {
                    let toslink_gate = if let Ok(gate) = toslink_switch_gate_render.lock() {
                        match *gate {
                            0 => "Off",
                            1 => "PS4",
                            2 => "PC",
                            _ => "Unknown",
                        }
                    }
                    else {
                        "Unavailable"
                    };
                    let chrome_gate = if let Ok(gate) = chrome_device_gate_render.lock() {
                        if *gate {
                        // if gate.get() {
                            "On"
                        }
                        else {
                            "Off"
                        }
                    } else {
                        "Unavailable"
                    };
                    let mut response = Response::with((status::Ok,
format!(r#"
<!doctype html>
<html>
<head></head>
<body>
<h2>Tessel Music Server</h2>

<form method="post">
<p>Toslink <button type="submit" name="toslink" value="toslink">{}</button></p>
<p>Music to Chat <button type="submit" name="music">On</button></p>
<p>Chrome to Chat <button type="submit" name="chrome" value="chrome">{}</button></p>
<br />
<button type="submit" name="shutdown" value="shutdown">Shutdown</button>
</form>
</body>
</html>
"#, toslink_gate, chrome_gate)
                    ));
                    response.headers.set(ContentType(Mime(TopLevel::Text, SubLevel::Html, vec![])));
                    Ok(response)
                }
            };

            let renderIndex = renderFactory();
            let index = move |_: &mut Request| {
                renderIndex()
            };

            let should_shutdown_condition = Arc::new(Condvar::new());

            let renderPostIndex = renderFactory();
            let should_shutdown_condition_post = should_shutdown_condition.clone();
            let should_shutdown_mutex_post = should_shutdown_mutex_http.clone();
            let toslink_switch_gate_http = toslink_switch_gate_http.clone();
            let chrome_device_gate_http = chrome_device_gate_http.clone();
            let postIndex = move |req: &mut Request| {
                let mut body_vec = Vec::new();
                req.body.read_to_end(&mut body_vec).unwrap();
                let body = String::from_utf8(body_vec).unwrap();
                println!("body {:?}", body);
                if body.contains("chrome") {
                    if let Ok(mut gate) = chrome_device_gate_http.lock() {
                        *gate = !*gate;
                        // gate.set(!gate.get());
                    }
                }
                else if body.contains("toslink") {
                    if let Ok(mut gate) = toslink_switch_gate_http.lock() {
                        *gate = match *gate {
                            0 => 1,
                            1 => 2,
                            2 => 0,
                            _ => 0,
                        };
                    }
                }
                else if body.contains("shutdown") {
                    let mut should_shutdown = should_shutdown_mutex_post.lock().unwrap();
                    *should_shutdown = true;
                    should_shutdown_condition_post.notify_one();
                }
                renderPostIndex()
            };

            let music = http_audio_stream_factory("music", http_music_mutex_clone, should_shutdown_mutex_http.clone(), net_music_tick_pair_clone.clone());
            let chrome = http_audio_stream_factory("chrome", http_chrome_mutex_clone, should_shutdown_mutex_http.clone(), net_chrome_tick_pair_clone.clone());

            match Iron::new(router!(
                index: get "/" => index,
                postIndex: post "/" => postIndex,
                music: post "/music" => music,
                chrome: post "/chrome" => chrome,
            )).listen_with("0.0.0.0:80", 8, Protocol::Http, Some(Timeouts {
                // read: Some(Duration::from_millis(10)),
                ..Default::default()
            })) {
                Ok(mut listening) => {
                    let mut should_shutdown = should_shutdown_mutex_http.lock().unwrap();
                    while !*should_shutdown {
                        should_shutdown = should_shutdown_condition.wait(should_shutdown).unwrap();
                    }
                    listening.close().unwrap();
                    {
                        let start = Instant::now();
                        loop {
                            if Instant::now().duration_since(start).as_secs() > 0 {
                                break;
                            }
                            yield_now();
                        }
                    }
                    process::exit(0);
                },
                Err(err) => {
                    println!("{:?}", err);
                    if let Ok(mut should_shutdown) = should_shutdown_mutex_http.lock() {
                        *should_shutdown = true;
                    }
                },
            }
        });

        let music = tcp_audio_stream_factory("music", tcp_music_mutex_clone, should_shutdown_mutex.clone(), net_music_tick_pair.clone());
        let chrome = tcp_audio_stream_factory("chrome", tcp_chrome_mutex_clone, should_shutdown_mutex.clone(), net_chrome_tick_pair.clone());

        thread::spawn(move || {
            let listener = TcpListener::bind("0.0.0.0:7777").unwrap();
            // let music = &music;
            for stream in listener.incoming() {
                match stream {
                    Ok(stream) => {
                        music(stream);
                    },
                    Err(_) => {},
                }
            }
        });

        thread::spawn(move || {
            let listener = TcpListener::bind("0.0.0.0:7778").unwrap();
            for stream in listener.incoming() {
                match stream {
                    Ok(stream) => {
                        chrome(stream);
                    },
                    Err(_) => {},
                }
            }
        });

        let mut steps = 0;

        let start = Instant::now();
        let mut net_start = false;
        let mut last_hint_run = Instant::now();
        let mut last_net_tick = last_hint_run;
        loop {
            if let Ok(should_shutdown) = should_shutdown_mutex.lock() {
                if *should_shutdown {
                    hint_condvar.notify_one();
                    (*net_music_tick_pair).1.notify_all();
                    (*net_chrome_tick_pair).1.notify_all();
                    break;
                }
            }
            // sleep(Duration::from_millis(1));
            // if steps % 10 == 0 {
            //     yield_now();
            // }
            // if steps > 100 {
                yield_now();
            // }
            if steps > 100 {
                let now = Instant::now();
                // sleep(Duration::from_millis(1));
                // yield_now();
                graph.update();
                print!("{} ", Instant::now().duration_since(now).subsec_nanos());
                steps = 0;
            }
            else {
                // sleep(Duration::from_millis(1));
                // yield_now();
                graph.update();
            }
            steps += 1;
            {
                let now = Instant::now();
                if now.duration_since(last_hint_run).as_secs() > 1 {
                    hint_condvar.notify_one();
                    last_hint_run = now;
                    yield_now();
                }
                if now.duration_since(last_net_tick).subsec_nanos() > 1000000 {
                //     // println!("notify music");
                //     {
                //         let music_in_guard = net_music_tick_pair.2.lock().unwrap();
                //         // println!("lock music in playing? {:?}", *music_in_guard);
                //         if *music_in_guard {
                            {
                                // let music_guard = net_music_tick_pair.0.lock().unwrap();
                //                 // println!("lock inner mutex");
                                net_music_tick_pair.1.notify_one();
                                yield_now();
                //                 // println!("notify inner condition");
                            }
                //             net_music_tick_pair.3.wait(music_in_guard).unwrap();
                //             // println!("returned from music");
                //         }
                //     }
                //     {
                //         let chrome_in_guard = net_chrome_tick_pair.2.lock().unwrap();
                //         if *chrome_in_guard {
                //             {
                //                 let chrome_guard = net_chrome_tick_pair.0.lock().unwrap();
                                net_chrome_tick_pair.1.notify_one();
                                yield_now();
                //             }
                //             net_chrome_tick_pair.3.wait(chrome_in_guard).unwrap();
                //         }
                //     }
                //     // (*net_music_tick_pair).1.notify_one();
                //     // (*net_chrome_tick_pair).1.notify_all();
                    last_net_tick = now;
                    // yield_now();
                }
                if !net_start && now.duration_since(start).as_secs() > 6 {
                    net_start = true;
                    let &(_, ref condvar) = &*net_start_pair;
                    condvar.notify_one();
                    yield_now();
                }
            }
        }

        // {
        //     let start = Instant::now();
        //     loop {
        //         if Instant::now().duration_since(start).as_secs() > 0 {
        //             break;
        //         }
        //         yield_now();
        //     }
        // }
        // process::exit(0);
    }
}
