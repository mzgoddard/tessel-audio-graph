//! A blinky example for Tessel

// Import the tessel library
extern crate tessel;
// Import the libusb library
// extern crate libusb;
// Import the alsa library
extern crate alsa;
// Import the graph_utils library
extern crate graph_utils;
// Import the graph_nodes library
extern crate graph_nodes;
// Import the tessel_audio_graph library
extern crate tessel_audio_graph;
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
use std::ops::Range;

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

use iron::prelude::*;
use iron::status;
use iron::headers::{Headers, ContentType};
use iron::mime::{Mime, TopLevel, SubLevel};
use iron::request;
use iron::{Timeouts, Protocol};

// use hyper::http::h1::HttpReader;

use router::*;

use graph_utils::*;
use graph_nodes::*;
use tessel_audio_graph::*;

// mod alsa_graph;

// use self::alsa_graph::*;

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

    let mut tessel = Tessel::new();

    // Turn on one of the LEDs
    // tessel.led[2].on().unwrap();

    // println!("I'm blinking! (Press CTRL + C to stop)");

    // let mut context = libusb::Context::new().unwrap();

    // for mut device in context.devices().unwrap().iter() {
    //     let device_desc = device.device_descriptor().unwrap();
    //
    //     println!("Bus {:03} Device {:03} ID {:04x}:{:04x}:{:02x}",
    //         device.bus_number(),
    //         device.address(),
    //         device_desc.vendor_id(),
    //         device_desc.product_id(),
    //         device_desc.class_code());
    // }

    // for t in &["ctl", "rawmidi", "timer", "seq", "hwdep"] {
    //     println!("{} devices:", t);
    //     let i = HintIter::new(None, &*CString::new(*t).unwrap()).unwrap();
    //     for a in i { println!("  {:?}", a) }
    // }

    let should_shutdown_mutex = Arc::new(Mutex::new(false));

    let mut graph = Graph::new();

    let activation_controller = ActivationController::new();

    let mut alsa_factory = AlsaFactory::new(activation_controller.clone());

    let alsa_factory_view = alsa_factory.view();
    let alsa_playback = |card| {
        alsa_factory_view.playback(card)
    };

    let alsa_factory_view = alsa_factory.view();
    let alsa_capture = |card| {
        alsa_factory_view.capture(card)
    };

    let duck = |peak, duck_state| {
        Duck::new(peak, duck_state)
    };

    let ducked = |states, volume| {
        Ducked::new(states, volume)
    };

    let volume = |volume| {
        Volume::new(volume)
    };

    let lean = |max_amount| {
        Box::new(Callback::new(Box::new(move |input, output| {
            let mut avail = min(input.len(), max_amount);
            output.write_from_ring(avail, input);
            input.clear();
        })))
    };

    let mono_to_stereo = || {
        MonoToStereo::new()
    };

    let gated = |state| {
        Gated::new(state)
    };

    let switch_gated = |state, my_state| {
        Switched::new(state, my_state)
    };

    let r48_to_r44 = || {
        Rate::new(48000, 44100)
    };

    let r44_to_r48 = || {
        Rate::new(44100, 48000)
    };

    let meter = |mut tessel: Tessel| {
        let mut peak = 0;
        let mut render_last = Instant::now();
        // roughly -30db, -24db, -18db, -12db
        let levels = vec!(1024, 2048, 4096, 8192);
        Box::new(Callback::new(Box::new(move |input, output| {
            let avail = input.len();
            let now = Instant::now();
            for (i, o) in input.read_slice(avail).iter().zip(output.write_slice(avail).iter_mut()) {
                *o = *i;
                if *i > peak {
                    peak = *i;
                }
            }
            // These led calls seem to either be expensive or blocking so we can't update very often.
            if now.duration_since(render_last).subsec_nanos() > 33000000 {
                render_last = now;
                for (led, level) in tessel.led.iter_mut().zip(levels.iter()) {
                    if peak > *level {
                        led.on();
                    }
                    else {
                        led.off();
                    }
                }
                peak = 0;
            }
        })))
    };

    let toslink_out_id = graph.connect(alsa_playback(AlsaCard {
        debug_name: "toslink",
        alsa_hint: AlsaLongName("USB Sound Device at usb-101c0000.ehci-1.2, full speed"),
        hw_params: AlsaHwParams::new_32ms(),
        sw_params: AlsaSwParams::new_16ms(),
        ..Default::default()
    }), GraphNodeParams { ..Default::default() });

    let toslink_switch_gate = SwitchState::new();

    let toslink_1_switch_id = graph.connect(switch_gated(toslink_switch_gate.clone(), 1), GraphNodeParams {
        to: vec!(toslink_out_id),
        ..Default::default()
    });

    let toslink_in_id = graph.connect(alsa_capture(AlsaCard {
        debug_name: "PS4 toslink",
        alsa_hint: AlsaLongName("USB Sound Device at usb-101c0000.ehci-1.2, full speed"),
        hw_params: AlsaHwParams::new_32ms(),
        sw_params: AlsaSwParams::new_2ms(),
        hctl: vec![
            ("PCM Capture Source", vec!((0, HCtlValue::Enumerated(2)))),
        ].into_iter().collect(),
        ..Default::default()
    }), GraphNodeParams {
        to: vec!(toslink_1_switch_id),
        ..Default::default()
    });

    let toslink_2_switch_id = graph.connect(switch_gated(toslink_switch_gate.clone(), 2), GraphNodeParams {
        to: vec!(toslink_out_id),
        ..Default::default()
    });

    let toslink_2_in_id = graph.connect(alsa_capture(AlsaCard {
        debug_name: "PC toslink",
        alsa_hint: AlsaLongName("USB Sound Device at usb-101c0000.ehci-1.1.2.1, full speed"),
        hw_params: AlsaHwParams::new_32ms(),
        sw_params: AlsaSwParams::new_2ms(),
        hctl: vec![
            ("PCM Capture Source", vec!((0, HCtlValue::Enumerated(2)))),
        ].into_iter().collect(),
        ..Default::default()
    }), GraphNodeParams {
        to: vec!(toslink_2_switch_id),
        ..Default::default()
    });

    let device_out_id = graph.connect(alsa_playback(AlsaCard {
        debug_name: "PS4 Chat",
        alsa_hint: AlsaLongName("USB Sound Device at usb-101c0000.ehci-1.1.1, full speed"),
        hw_params: AlsaHwParams::new_44100hz_64ms(),
        sw_params: AlsaSwParams::new_32ms(),
        ..Default::default()
    }), GraphNodeParams { ..Default::default() });

    let device_2_out_id = graph.connect(alsa_playback(AlsaCard {
        debug_name: "PC Chat",
        alsa_hint: AlsaLongName("USB Sound Device at usb-101c0000.ehci-1.1.4.3.1, full speed"),
        hw_params: AlsaHwParams::new_44100hz_64ms(),
        sw_params: AlsaSwParams::new_32ms(),
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

    let transmitter_out_id = graph.connect(alsa_playback(AlsaCard {
        debug_name: "transmitter",
        alsa_hint: AlsaLongName("Astro Gaming Inc. ASTRO Wireless Transmitter at usb-101c0000.ehci-1.1.4.1, full"),
        hw_params: AlsaHwParams::new_32ms(),
        sw_params: AlsaSwParams::new_4ms(),
        ..Default::default()
    }), GraphNodeParams { ..Default::default() });

    let transmitter_lean_id = graph.connect(lean(768), GraphNodeParams {
        to: vec!(transmitter_out_id),
        ..Default::default()
    });

    let office_out_id = graph.connect(alsa_playback(AlsaCard {
        debug_name: "office",
        alsa_hint: AlsaLongName("C-Media Electronics Inc. USB Audio Device at usb-101c0000.ehci-1.1, full speed"),
        hw_params: AlsaHwParams::new_44100hz_32ms(),
        sw_params: AlsaSwParams::new_4ms(),
        ..Default::default()
    }), Default::default());

    let office_r48_id = graph.connect(r48_to_r44(), GraphNodeParams {
        to: vec!(office_out_id),
        ..Default::default()
    });

    // let transmitter_mix_id = graph.connect(Box::new(BaseMix::new()), GraphNodeParams {
    //     to: vec!(transmitter_lean_id, office_r48_id),
    //     ..Default::default()
    // });

    let transmitter_mix_id = graph.connect(meter(tessel), GraphNodeParams {
        to: vec!(transmitter_lean_id, office_r48_id),
        ..Default::default()
    });

    let device_duck_state = DuckState::new();

    let device_duck_in_id = graph.connect(duck(1000, device_duck_state.clone()), GraphNodeParams {
        to: vec!(transmitter_mix_id),
        ..Default::default()
    });

    let device_in_44_to_48 = graph.connect(r44_to_r48(), GraphNodeParams {
        to: vec!(device_duck_in_id),
        ..Default::default()
    });

    let device_in_id = graph.connect(alsa_capture(AlsaCard {
        debug_name: "PS4 Chat",
        alsa_hint: AlsaLongName("USB Sound Device at usb-101c0000.ehci-1.1.1, full speed"),
        hw_params: AlsaHwParams::new_44100hz_32ms(),
        sw_params: AlsaSwParams::new_16ms(),
        hctl: vec![
            ("PCM Capture Source", vec!((0, HCtlValue::Enumerated(0)))),
        ].into_iter().collect(),
        ..Default::default()
    }), GraphNodeParams {
        to: vec!(device_in_44_to_48),
        ..Default::default()
    });

    let device_2_in_44_to_48 = graph.connect(r44_to_r48(), GraphNodeParams {
        to: vec!(device_duck_in_id),
        ..Default::default()
    });

    let device_2_in_id = graph.connect(alsa_capture(AlsaCard {
        debug_name: "PC Chat",
        alsa_hint: AlsaLongName("USB Sound Device at usb-101c0000.ehci-1.1.4.3.1, full speed"),
        hw_params: AlsaHwParams::new_32ms(),
        sw_params: AlsaSwParams::new_16ms(),
        hctl: vec![
            ("PCM Capture Source", vec!((0, HCtlValue::Enumerated(1)))),
        ].into_iter().collect(),
        ..Default::default()
    }), GraphNodeParams {
        to: vec!(device_2_in_44_to_48),
        ..Default::default()
    });

    let mic_duck_state = DuckState::new();

    let mic_in_duck = graph.connect(duck(5500, device_duck_state.clone()), GraphNodeParams {
        to: vec!(transmitter_mix_id, device_mix_id),
        ..Default::default()
    });

    let streammic_in_id = graph.connect(alsa_capture(AlsaCard {
        debug_name: "Stream Mic",
        alsa_hint: AlsaLongName("Turtle Beach Turtle Beach Stream Mic (Mic On at usb-101c0000.ehci-1.1.4.4.1, fu"),
        hw_params: AlsaHwParams::new_32ms(),
        sw_params: AlsaSwParams::new_2ms(),
        ..Default::default()
    }), GraphNodeParams {
        to: vec!(mic_in_duck),
        ..Default::default()
    });

    let transmitter_stereo_id = graph.connect(mono_to_stereo(), GraphNodeParams {
        to: vec!(mic_in_duck),
        ..Default::default()
    });

    let transmitter_in_id = graph.connect(alsa_capture(AlsaCard {
        debug_name: "transmitter",
        alsa_hint: AlsaLongName("Astro Gaming Inc. ASTRO Wireless Transmitter at usb-101c0000.ehci-1.1.4.1, full"),
        hw_params: AlsaHwParams::new_mono_32ms(),
        sw_params: AlsaSwParams::new_2ms(),
        ..Default::default()
    }), GraphNodeParams {
        to: vec!(transmitter_stereo_id),
        ..Default::default()
    });

    let content_duck_id = graph.connect(ducked(vec!(mic_duck_state, device_duck_state), (1, 5)), GraphNodeParams {
        to: vec!(transmitter_mix_id),
        ..Default::default()
    });

    let mut music_buffer = IoNodeBuffer::new("music", activation_controller.clone());
    let http_music_in_id = graph.connect(music_buffer.capture(), GraphNodeParams {
        to: vec!(content_duck_id),
        ..Default::default()
    });

    let chrome_device_gate = GateState::new();
    let chrome_gated_id = graph.connect(gated(chrome_device_gate.clone()), GraphNodeParams {
        to: vec!(device_mix_id),
        ..Default::default()
    });

    let mut chrome_buffer = IoNodeBuffer::new("chrome", activation_controller.clone());
    let http_chrome_in_id = graph.connect(chrome_buffer.capture(), GraphNodeParams {
        to: vec!(content_duck_id, chrome_gated_id),
        ..Default::default()
    });

    let music_http = {
        let cb = music_buffer.read_factory().reader();
        move |req: &mut Request| {
            cb(&mut req.body);

            Ok(Response::with((status::Ok, "Ok")))
        }
    };

    let chrome_http = {
        let cb = chrome_buffer.read_factory().reader();
        move |req: &mut Request| {
            cb(&mut req.body);

            Ok(Response::with((status::Ok, "Ok")))
        }
    };

    let should_shutdown_mutex_http = should_shutdown_mutex.clone();
    let toslink_switch_gate_http = toslink_switch_gate.clone();
    let chrome_device_gate_http = chrome_device_gate.clone();

    let net_start_pair = Arc::new((Mutex::new(false), Condvar::new()));
    let net_start_pair_clone = net_start_pair.clone();

    thread::spawn(move || {
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
                let toslink_gate = match toslink_switch_gate_render.get() {
                    0 => "Off",
                    1 => "PS4",
                    2 => "PC",
                    _ => "Unknown",
                };
                let chrome_gate = match chrome_device_gate_render.get() {
                    true => "On",
                    false => "Off",
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
                chrome_device_gate_http.toggle();
            }
            else if body.contains("toslink") {
                toslink_switch_gate_http.map(|state| match *state {
                    0 => 1,
                    1 => 2,
                    2 => 0,
                    _ => 0,
                });
            }
            else if body.contains("shutdown") {
                let mut should_shutdown = should_shutdown_mutex_post.lock().unwrap();
                *should_shutdown = true;
                should_shutdown_condition_post.notify_one();
            }
            renderPostIndex()
        };

        match Iron::new(router!(
            index: get "/" => index,
            postIndex: post "/" => postIndex,
            music: post "/music" => music_http,
            chrome: post "/chrome" => chrome_http,
        )).listen_with("0.0.0.0:80", 8, Protocol::Http, Some(Timeouts {
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

    let cb = music_buffer.read_factory().reader();
    let music_tcp = move |mut stream: TcpStream| {
        stream.set_nonblocking(true).unwrap();
        cb(&mut stream);
    };

    thread::spawn(move || {
        let listener = TcpListener::bind("0.0.0.0:7777").unwrap();
        for stream in listener.incoming() {
            match stream {
                Ok(stream) => {
                    music_tcp(stream);
                },
                Err(_) => {},
            }
        }
    });

    let cb = chrome_buffer.read_factory().reader();
    let chrome_tcp = move |mut stream: TcpStream| {
        stream.set_nonblocking(true).unwrap();
        cb(&mut stream);
    };

    thread::spawn(move || {
        let listener = TcpListener::bind("0.0.0.0:7778").unwrap();
        for stream in listener.incoming() {
            match stream {
                Ok(stream) => {
                    chrome_tcp(stream);
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
                // Breaking the loop will cause us to exit the scope owning alsa_factory and music_buffer
                // leading to their own shutdown signals being executed.
                break;
            }
        }

        yield_now();

        if steps > 100 {
            let now = Instant::now();
            graph.update();
            let ns = Instant::now().duration_since(now).subsec_nanos();
            if ns > 1000000 {
                println!("{} ", ns);
            }
            steps = 0;
        }
        else {
            graph.update();
        }
        steps += 1;

        {
            let now = Instant::now();

            alsa_factory.update(now);
            music_buffer.update(now);
            chrome_buffer.update(now);

            if !net_start && now.duration_since(start).as_secs() > 6 {
                net_start = true;
                let &(_, ref condvar) = &*net_start_pair;
                condvar.notify_one();
            }
        }
    }
}
