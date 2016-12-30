use super::{Node, RingBuffer, copy_out_ring};

type CaptureFn = Box<FnMut(&mut RingBuffer)>;

pub struct Capture {
    tmp_state: Option<(RingBuffer, Vec<i16>)>,
    callback: CaptureFn,
}

impl Capture {
    pub fn new(callback: CaptureFn) -> Capture {
        Capture {
            tmp_state: Some((RingBuffer::new(), Vec::<i16>::new())),
            callback: callback,
        }
    }
}

impl Node for Capture {
    fn update(&mut self, _: &mut [RingBuffer], outputs: &mut [RingBuffer]) {
        let (mut ring, mut buffer) = self.tmp_state.take().unwrap();
        (self.callback)(&mut ring);
        let avail = ring.len();
        // ring.read_into(avail, &mut buffer);
        // copy_out(avail, &buffer, outputs);
        copy_out_ring(avail, &mut buffer, &mut ring, outputs);
        self.tmp_state = Some((ring, buffer));
    }
}

#[cfg(test)]
mod test {
    use super::Capture;
    use super::super::{Node, RingBuffer};

    #[test]
    fn it_captures() {
        let mut a = {
            let v1 = (48..96).map(|x| x as i16).collect::<Vec<i16>>();
            Capture::new(Box::new(move |ring| {
                ring.write_from(48, &v1);
            }))
        };
        let mut inputs = vec!();
        let mut outputs = vec!(RingBuffer::new());
        {
            let n = &mut a as &mut Node;
            n.update(&mut inputs, &mut outputs);
        }
        let mut o1 = Vec::<i16>::new();
        let avail = outputs[0].len();
        outputs[0].read_into(avail, &mut o1);
        assert_eq!(o1[0], 48);
    }
}
