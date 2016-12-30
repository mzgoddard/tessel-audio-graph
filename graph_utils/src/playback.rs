use super::{Node, RingBuffer, BaseMix};

type PlaybackFn = Box<FnMut(&mut RingBuffer)>;

pub struct Playback {
    base_mix: BaseMix,
    tmp_state: Option<RingBuffer>,
    callback: PlaybackFn,
}

impl Playback {
    pub fn new(callback: PlaybackFn) -> Playback {
        Playback {
            base_mix: BaseMix::new(),
            tmp_state: Some(RingBuffer::new()),
            callback: callback,
        }
    }
}

impl Node for Playback {
    fn update(&mut self, inputs: &mut [RingBuffer], _: &mut [RingBuffer]) {
        let mut ring = self.tmp_state.take().unwrap();
        self.base_mix.mix_inputs_ring(inputs, &mut ring);
        (self.callback)(&mut ring);
        self.tmp_state = Some(ring);
    }
}

#[cfg(test)]
mod test {
    use super::Playback;
    use super::super::{Node, RingBuffer};

    #[test]
    fn it_plays_back() {
        let mut a = {
            let mut buffer = Vec::<i16>::new();
            Playback::new(Box::new(move |ring| {
                let avail = ring.len();
                ring.read_into(avail, &mut buffer);
            }))
        };
        let mut inputs = vec!(RingBuffer::new());
        let v1 = (48..96).map(|x| x as i16).collect::<Vec<i16>>();
        inputs[0].write_from(48, &v1);
        let mut outputs = vec!();
        {
            let n = &mut a as &mut Node;
            n.update(&mut inputs, &mut outputs);
        }
        assert_eq!(inputs[0].len(), 0);
    }
}
