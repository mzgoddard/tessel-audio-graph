//! A simple example for alsa sound passthrough

// Import the graph_utils library
extern crate graph_utils;
// Import the graph_nodes library
extern crate graph_nodes;
// Import the tessel_audio_graph library
extern crate tessel_audio_graph;

use std::thread::yield_now;
use std::time::Instant;

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
        hw_params: AlsaHwParams::new_44100hz_8ms(),
        sw_params: AlsaSwParams::new_4ms(),
        ..Default::default()
    }), Default::default());

    let simple_stereo_id = graph.connect(MonoToStereo::new(), GraphNodeParams {
        to: vec!(simple_out_id),
        ..Default::default()
    });

    graph.connect(alsa_factory.view().capture(AlsaCard {
        debug_name: "simple",
        alsa_hint: AlsaUsbPort("usb-101c0000.ehci-1.1"),
        hw_params: AlsaHwParams {
            channels: 1,
            ..AlsaHwParams::new_44100hz_8ms()
        },
        sw_params: AlsaSwParams::new_4ms(),
        ..Default::default()
    }), GraphNodeParams {
        to: vec!(simple_stereo_id),
        ..Default::default()
    });

    loop {
        yield_now();
        graph.update();

        let now = Instant::now();
        alsa_factory.update(now);
    }
}
