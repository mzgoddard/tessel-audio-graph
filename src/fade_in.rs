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
