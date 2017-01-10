//! A tcp-using example for Tessel with alsa sound support

extern crate tessel;
// Import the graph_utils library
extern crate graph_utils;
// Import the graph_nodes library
extern crate graph_nodes;
// Import the tessel_audio_graph library
extern crate tessel_audio_graph;

use std::net::{TcpListener, TcpStream};
use std::thread::yield_now;
use std::thread;
use std::time::Instant;

use tessel::*;

use graph_utils::*;
use graph_nodes::*;
use tessel_audio_graph::*;

fn main() {
    let mut graph = Graph::new();

    let activation_controller = ActivationController::new();

    let mut alsa_factory = AlsaFactory::new(activation_controller.clone());

    let simple_out_id = graph.connect(alsa_factory.view().playback(AlsaCard {
        debug_name: "simple",
        alsa_hint: AlsaUsbPort("usb-101c0000.ehci-1.1"),
        hw_params: AlsaHwParams::new_44100hz_64ms(),
        sw_params: AlsaSwParams::new_ms(32),
        ..Default::default()
    }), Default::default());

    let simple_r44_id = graph.connect(Rate::new(48000, 44100), GraphNodeParams {
        to: vec!(simple_out_id),
        ..Default::default()
    });

    let meter_id = graph.connect(LedMeter::new(Tessel::new()), GraphNodeParams {
        to: vec!(simple_r44_id),
        ..Default::default()
    });

    let mut music_buffer = IoNodeBuffer::new("music", activation_controller.clone());
    graph.connect(music_buffer.capture(), GraphNodeParams {
        to: vec!(meter_id),
        ..Default::default()
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

    loop {
        yield_now();
        graph.update();

        let now = Instant::now();

        alsa_factory.update(now);
        music_buffer.update(now);
    }
}
