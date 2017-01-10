use std::sync::{Arc, Mutex};
use std::thread;
use std::thread::yield_now;
use std::time::Instant;

use graph_utils::*;

use tessel::*;

pub struct LedMeter(Callback);

impl CallbackInner for LedMeter {
    fn get_callback(&mut self) -> &mut Callback {
        &mut self.0
    }
}

impl LedMeter {
    pub fn new(mut tessel: Tessel) -> Box<LedMeter> {
        let mut peak = 0;
        let mut peak_last = Instant::now();
        let peak_mutex = Arc::new(Mutex::new(0));

        {
            // roughly -30db, -24db, -18db, -12db
            let levels = vec!(1024, 2048, 4096, 8192);
            let mut render_last = Instant::now();
            let peak_mutex = Arc::downgrade(&peak_mutex);

            thread::spawn(move || {
                loop {
                    // These led calls seem to either be expensive or blocking so we can't update very often.
                    let now = Instant::now();
                    if now.duration_since(render_last).subsec_nanos() > 8000000 {
                        let peak = if let Some(mutex) = peak_mutex.upgrade() {
                            match mutex.lock() {
                                Ok(guard) => *guard,
                                Err(_) => {
                                    println!("poisoned meter mutex ...");
                                    break;
                                },
                            }
                        }
                        else {
                            println!("meter renderer shutting down ...");
                            break;
                        };

                        render_last = now;
                        for (led, level) in tessel.led.iter_mut().zip(levels.iter()) {
                            if peak >= *level && !led.read() {
                                led.on().unwrap();
                            }
                            else if peak < *level && led.read() {
                                led.off().unwrap();
                            }
                        }
                    }

                    yield_now();
                }
            });
        }

        // This is currently an incredibly naive algorithm. Instead of resetting should probably fill a
        // buffer and use a Biquad or FFT to target a frequency range and return the peak of that range for
        // the desired window.
        Box::new(LedMeter(Callback::new(Box::new(move |input, output| {
            let now = Instant::now();
            if now.duration_since(peak_last).subsec_nanos() > 8000000 {
                peak = 0;
                peak_last = now;
            }

            let avail = input.len();
            for (i, o) in input.read_slice(avail).iter().zip(output.write_slice(avail).iter_mut()) {
                *o = *i;
                if *i > peak {
                    peak = *i;
                }
            }

            if let Ok(mut guard) = peak_mutex.try_lock() {
                *guard = peak;
            }
        }))))
    }
}
