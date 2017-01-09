use std::ops::Range;
use std::sync::{Arc, Mutex, MutexGuard, Condvar};
use std::thread;

use std::iter::Iterator;

use std::ffi::CString;

use std::collections::btree_map::BTreeMap;

use std::time::Instant;

use std::slice;

use std::cmp::min;

use alsa;

use alsa::device_name::HintIter;
use alsa::card;
use alsa::card::Card;

use alsa::{Direction, ValueOr, Result};
use alsa::ctl::{Ctl, ElemType, ElemValue};
use alsa::hctl::{HCtl, Elem};
use alsa::pcm::{PCM, HwParams, SwParams, Format, Access, State};

use activation::*;

use graph_utils::{Capture, Playback, RingBuffer};

pub enum AlsaCardHint {
    AlsaNone,
    // UsbPort("usb-101c0000.ehci-1.2")
    AlsaUsbPort(&'static str),
    // Name("USB Sound Device")
    AlsaName(&'static str),
    // LongName("USB Sound Device at usb-101c0000.ehci-1.2, full speed")
    AlsaLongName(&'static str),
}

pub use self::AlsaCardHint::*;

impl AlsaCardHint {
    fn match_longname(&self, longname: &String) -> bool {
        match self {
            &AlsaNone => false,
            &AlsaUsbPort(port) => {
                if let Some(index) = longname.find(port) {
                    let s = &longname[(index + port.len())..(index + port.len() + 1)];
                    s == ","
                }
                else {false}
            },
            &AlsaName(n) => longname.starts_with(n),
            &AlsaLongName(ln) => longname == ln,
        }
    }
}

pub struct AlsaHwParams {
    pub channels: u32,
    pub rate: u32,
    pub format: Format,
    pub access: Access,
    pub periods: u32,
    pub period_size: i32,
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
    pub fn new_32ms() -> AlsaHwParams {
        AlsaHwParams {
            periods: 32,
            ..Default::default()
        }
    }

    pub fn new_64ms() -> AlsaHwParams {
        AlsaHwParams {
            periods: 32,
            ..Default::default()
        }
    }

    pub fn new_mono_32ms() -> AlsaHwParams {
        AlsaHwParams {
            channels: 1,
            periods: 32,
            ..Default::default()
        }
    }

    pub fn new_44100hz_64ms() -> AlsaHwParams {
        AlsaHwParams {
            rate: 44100,
            periods: 32,
            ..Default::default()
        }
    }

    pub fn new_44100hz_32ms() -> AlsaHwParams {
        AlsaHwParams {
            rate: 44100,
            periods: 32,
            ..Default::default()
        }
    }

    pub fn set_params(&self, pcm: &PCM) -> alsa::Result<()> {
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

pub struct AlsaSwParams {
    pub avail_min: i32,
    pub start_threshold: i32,
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
    pub fn new_2ms() -> AlsaSwParams {
        AlsaSwParams {
            avail_min: 48 * 2,
            start_threshold: 48 * 2,
        }
    }

    pub fn new_4ms() -> AlsaSwParams {
        AlsaSwParams {
            avail_min: 48 * 4,
            start_threshold: 48 * 4,
        }
    }

    pub fn new_16ms() -> AlsaSwParams {
        AlsaSwParams {
            avail_min: 48 * 16,
            start_threshold: 48 * 16,
        }
    }

    pub fn new_32ms() -> AlsaSwParams {
        AlsaSwParams {
            avail_min: 48 * 32,
            start_threshold: 48 * 32,
        }
    }

    pub fn set_params(&self, pcm: &PCM) {
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

pub struct AlsaCard {
    pub debug_name: &'static str,
    pub pcm_hint: &'static str,
    pub alsa_hint: AlsaCardHint,
    pub pcm_device: usize,
    pub hw_params: AlsaHwParams,
    pub sw_params: AlsaSwParams,
    pub hctl: BTreeMap<&'static str, Vec<(u32, HCtlValue)>>,
}

impl Default for AlsaCard {
    fn default() -> AlsaCard {
        AlsaCard {
            debug_name: "default",
            pcm_hint: "default",
            alsa_hint: AlsaNone,
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

struct AlsaCardListThread {
    should_shutdown: Arc<Mutex<bool>>,
    longnames: Arc<Mutex<Vec<String>>>,
    condvar: Arc<Condvar>,
}

impl AlsaCardListThread {
    fn run(self) {
        thread::spawn(move || {
            let mut longnames = self.longnames.lock().unwrap();
            loop {
                {
                    for _ in 0..(*longnames).len() {
                        (*longnames).pop().unwrap();
                    }
                    for card_result in card::Iter::new() {
                        if let Ok(card) = card_result {
                            if let Ok(ref longname) = card.get_longname() {
                                (*longnames).push(longname.clone());
                            }
                            else {
                                break;
                            }
                        }
                        else {
                            break;
                        }
                    }
                }

                longnames = self.condvar.wait(longnames).unwrap();

                if let Ok(should_shutdown) = self.should_shutdown.lock() {
                    if *should_shutdown {
                        break;
                    }
                }
            }
        });
    }
}

pub struct AlsaCardListInner {
    should_shutdown: Arc<Mutex<bool>>,
    last_poll: Instant,
    longnames: Arc<Mutex<Vec<String>>>,
    condvar: Arc<Condvar>,
}

impl Drop for AlsaCardListInner {
    fn drop(&mut self) {
        if let Ok(mut guard) = self.should_shutdown.lock() {
            *guard = true;
        }
        self.condvar.notify_all();
    }
}

impl AlsaCardListInner {
    pub fn new() -> AlsaCardListInner {
        let inner = AlsaCardListInner {
            should_shutdown: Arc::new(Mutex::new(false)),
            last_poll: Instant::now(),
            longnames: Arc::new(Mutex::new(Vec::new())),
            condvar: Arc::new(Condvar::new()),
        };
        inner.spawn();
        inner
    }

    pub fn view(&self) -> AlsaCardList {
        AlsaCardList {
            longnames: self.longnames.clone(),
        }
    }

    pub fn spawn(&self) {
        AlsaCardListThread {
            should_shutdown: self.should_shutdown.clone(),
            longnames: self.longnames.clone(),
            condvar: self.condvar.clone(),
        }.run();
    }

    pub fn update(&mut self, now: Instant) {
        if now.duration_since(self.last_poll).as_secs() > 1 {
            self.last_poll = now;
            self.condvar.notify_one();
        }
    }
}

#[derive(Clone)]
pub struct AlsaCardList {
    longnames: Arc<Mutex<Vec<String>>>,
}

impl AlsaCardList {
    pub fn iter<'a>(&'a self) -> AlsaCardIter<'a> {
        if let Ok(mut guard) = self.longnames.try_lock() {
            let len = guard.len();
            let tmp_vec = guard.split_off(0);
            AlsaCardIter {
                range: 0..len,
                maybe_guard: Some(guard),
                tmp_vec: tmp_vec,
            }
        }
        else {
            AlsaCardIter {
                range: 0..0,
                maybe_guard: None,
                tmp_vec: Vec::new(),
            }
        }
    }
}

struct AlsaCardIter<'a> {
    range: Range<usize>,
    maybe_guard: Option<MutexGuard<'a, Vec<String>>>,
    tmp_vec: Vec<String>,
    // longnames_iter: slice::Iter<'a,
}

impl<'a> Drop for AlsaCardIter<'a> {
    fn drop(&mut self) {
        if let Some(ref mut guard) = self.maybe_guard {
            let len = self.tmp_vec.len();
            for name in self.tmp_vec.drain(0..len) {
                guard.push(name);
            }
        }
    }
}

impl<'a> Iterator for AlsaCardIter<'a> {
    type Item = &'a String;
    fn next(&mut self) -> Option<Self::Item> {
        match self.range.next() {
            Some(i) => Some(unsafe { &*(&self.tmp_vec[i] as *const String) as &String }),
            None => None,
        }
    }
}

pub struct AlsaFactory {
    alsa_card_list_inner: AlsaCardListInner,
    activation_controller: ActivationController,
}

#[derive(Clone)]
pub struct AlsaFactoryView {
    alsa_card_list: AlsaCardList,
    activation_controller: ActivationController,
}

// struct AlsaFeatures {
//     alsa_card_list: AlsaCardList,
//     activation_controller: ActivationController,
// }

impl AlsaFactory {
    pub fn new(activation_controller: ActivationController) -> AlsaFactory {
        AlsaFactory {
            alsa_card_list_inner: AlsaCardListInner::new(),
            activation_controller: activation_controller,
        }
    }

    pub fn update(&mut self, now: Instant) {
        self.alsa_card_list_inner.update(now);
    }

    pub fn view(&self) -> AlsaFactoryView {
        AlsaFactoryView {
            alsa_card_list: self.alsa_card_list_inner.view(),
            activation_controller: self.activation_controller.clone(),
        }
    }
}

impl AlsaFactoryView {
    pub fn playback(&self, card: AlsaCard) -> Box<Playback> {
        let alsa_card_list = self.alsa_card_list.clone();
        let activation_controller_clone = self.activation_controller.clone();
        let mut activation_guard = None;

        let mut maybe_pcm_io = None;
        let mut buffer = Vec::new();
        let pcm_period = card.hw_params.period_size as usize;
        let pcm_max = pcm_period * card.hw_params.periods as usize;

        let mut cooloff = false;
        let mut cooloff_start = Instant::now();

        let mut paused = false;

        Box::new(Playback::new(Box::new(move |input| {
            if maybe_pcm_io.is_none() {
                if cooloff {
                    if Instant::now().duration_since(cooloff_start).as_secs() > 4 {
                        cooloff = false;
                    }
                    else {
                        return;
                    }
                }

                let hint = alsa_card_list.iter().enumerate().find(|&(i, hint)| {
                    card.alsa_hint.match_longname(hint) || *hint == card.pcm_hint
                }).map(|(i, _)| i);

                if hint.is_some() {
                    if activation_guard.is_none() {
                        activation_guard = activation_controller_clone.activate();
                        if activation_guard.is_some() {
                            println!("activating {:?} playback", card.debug_name);
                        }
                        return;
                    }
                }

                if let Some(index) = hint {
                    let card_id = {
                        let card = Card::new(index as i32);
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
                                println!("error setting hwparams {:?} playback", card.debug_name);
                                activation_guard = None;
                                println!("failed to activate {:?} playback", card.debug_name);
                                cooloff = true;
                                cooloff_start = Instant::now();
                            }
                            card.sw_params.set_params(&pcm);

                            println!("connect {:?} playback", card.debug_name);

                            activation_guard = None;
                            println!("activated {:?} playback", card.debug_name);

                            input.clear();
                            maybe_pcm_io = Some(pcm);
                        }
                        else {
                            activation_guard = None;
                            println!("failed to activate {:?} playback", card.debug_name);

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
                        println!("disconnected {:?} playback", card.debug_name);
                        unset = true;
                    }
                    else if status.get_state() == State::XRun {
                        if let Err(_) = pcm.prepare() {
                            println!("error trying to recover {:?} playback", card.debug_name);
                        }
                        if pcm.state() != State::Prepared {
                            println!("overrun {:?} playback", card.debug_name);
                            unset = true;
                        }
                    }
                    else if activation_controller_clone.is_activating().available() {
                        let activating = activation_controller_clone.is_activating();
                        if activating.activating() && !paused {
                            if let Ok(_) = pcm.pause(true) {}
                            paused = true;
                            println!("paused {:?} playback", card.debug_name);
                        }
                        else if activating.running() && paused {
                            if let Ok(_) = pcm.pause(false) {}
                            paused = false;
                            println!("resumed {:?} playback", card.debug_name);
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
                                println!("error writing {:?} playback", card.debug_name);
                                // unset =  true;
                            }
                            // println!("p{:?}", avail);
                        }
                        else {
                            println!("error creating io {:?} playback", card.debug_name);
                            // unset = true;
                        }
                    }
                    else {
                        // print!("pcm{:?}buffer{:?}", pcm_avail, buffer_avail);
                        // print!("p0");
                    }
                }
                else {
                    println!("error checking avail {:?} playback", card.debug_name);
                    // unset = true;
                }
            }
            if unset {
                maybe_pcm_io = None;
                cooloff = true;
                cooloff_start = Instant::now();
            }
        })))
    }

    pub fn capture(&self, card: AlsaCard) -> Box<Capture> {
        let alsa_card_list = self.alsa_card_list.clone();
        let activation_controller_clone = self.activation_controller.clone();
        let mut activation_guard = None;

        let mut active_capture = None;

        let mut cooloff = false;
        let mut cooloff_start = Instant::now();

        Box::new(Capture::new(Box::new(move |output| {
            output.active = false;
            if active_capture.is_none() {
                if cooloff {
                    if Instant::now().duration_since(cooloff_start).as_secs() > 4 {
                        cooloff = false;
                    }
                    else {
                        return;
                    }
                }

                let hint = alsa_card_list.iter().enumerate().find(|&(i, hint)| {
                    card.alsa_hint.match_longname(hint) || *hint == card.pcm_hint
                }).map(|(i, _)| i as i32);

                if hint.is_some() {
                    if activation_guard.is_none() {
                        activation_guard = activation_controller_clone.activate();
                        if activation_guard.is_some() {
                            println!("activating {:?} capture", card.debug_name);
                        }
                        return;
                    }
                }
                // println!("search {:?} {:?}", card.debug_name, Instant::now().duration_since(now));

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
                                println!("error setting hwparams {:?} capture", card.debug_name);
                                activation_guard = None;
                                println!("failed to activate {:?} capture", card.debug_name);
                                cooloff = true;
                                cooloff_start = Instant::now();
                            }
                            card.sw_params.set_params(&pcm);

                            println!("connect {:?} capture", card.debug_name);

                            activation_guard = None;
                            println!("activated {:?} capture", card.debug_name);

                            Some(pcm)
                        }
                        else {
                            activation_guard = None;
                            println!("failed to activate {:?} capture", card.debug_name);
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
                    let debug_name = card.debug_name.clone();
                    let num_channels = card.hw_params.channels as usize;
                    let format = card.hw_params.format;
                    let start_threshold = card.sw_params.start_threshold as usize;
                    let activation_controller_clone_clone = activation_controller_clone.clone();
                    let mut paused = false;

                    active_capture = Some(Box::new(move |output: &mut RingBuffer| {
                        let mut unset = false;
                        let mut cont = if let Ok(status) = pcm.status() {
                            if status.get_state() == State::Disconnected {
                                println!("disconnected {:?} capture", debug_name);
                                unset = true;
                                false
                            }
                            else if status.get_state() == State::XRun {
                                pcm.prepare().unwrap();
                                if pcm.state() != State::Prepared {
                                    println!("overrun {:?} capture", debug_name);
                                    unset = true;
                                    false
                                }
                                else {
                                    !paused
                                }
                            }
                            else if activation_controller_clone_clone.is_activating().available() {
                                let activating = activation_controller_clone_clone.is_activating();
                                if activating.activating() && !paused {
                                    if let Ok(_) = pcm.pause(true) {}
                                    paused = true;
                                    println!("paused {:?} capture", debug_name);
                                }
                                else if activating.running() && paused {
                                    if let Ok(_) = pcm.pause(false) {}
                                    paused = false;
                                    println!("resumed {:?} capture", debug_name);
                                    return false;
                                }
                                if status.get_state() == State::Prepared {
                                    if let Err(_) = pcm.start() {
                                        println!("error starting {:?} capture", debug_name);
                                    }
                                }
                                !paused
                            }
                            else {
                                if status.get_state() == State::Prepared {
                                    if let Err(_) = pcm.start() {
                                        println!("error starting {:?} capture", debug_name);
                                    }
                                }
                                !paused
                            }
                        }
                        else {
                            println!("error checking status {:?} capture", debug_name);
                            unset = true;
                            false
                        };
                        output.active = !paused;
                        let maybe_avail = if cont {
                            match pcm.avail().map(|x| x as usize) {
                                Ok(avail) => Some(avail),
                                Err(_) => {
                                    println!("error checking available {:?} capture", debug_name);
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
                                                println!("error creating io {:?} capture", debug_name);
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
                                                    println!("error reading {:?} capture", debug_name);
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
                                                println!("error creating io {:?} capture", debug_name);
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
                                                        println!("error reading {:?} capture", debug_name);
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
                                        println!("unknown format {:?} capture", debug_name);
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
    }
}
