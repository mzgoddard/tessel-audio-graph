use std::cmp::min;

#[derive(Debug)]
pub struct RingBuffer {
    pub max_length: usize,
    pub active: bool,
    start_index: usize,
    end_index: usize,
    pub buffer: Vec<i16>,
}

impl Default for RingBuffer {
    fn default() -> RingBuffer {
        RingBuffer {
            max_length: 32768,
            active: true,
            start_index: 0,
            end_index: 0,
            buffer: Vec::<i16>::new(),
        }
    }
}

impl RingBuffer {
    pub fn new() -> RingBuffer {
        RingBuffer { ..Default::default() }
    }

    pub fn from(buffer: Vec<i16>) -> RingBuffer {
        RingBuffer {
            end_index: buffer.len(),
            buffer: buffer,
            .. Default::default()
        }
    }

    pub fn len(&self) -> usize {
        if self.start_index > self.end_index {
            assert!(self.max_length + 1 - self.start_index + self.end_index >= 0);
            self.max_length + 1 - self.start_index + self.end_index
        }
        else {
            assert!(self.end_index - self.start_index >= 0);
            self.end_index - self.start_index
        }
    }

    pub fn clear(&mut self) {
        self.start_index = 0;
        self.end_index = 0;
    }

    pub fn read_into(&mut self, amount: usize, buffer: &mut Vec<i16>) -> usize {
        assert!(amount >= 0);
        let amount_avail = min(amount, self.len());
        for _ in (buffer.len())..amount_avail {
            buffer.push(0);
        }
        for i in 0..amount_avail {
            buffer[i] = self.buffer[self.start_index];
            self.start_index += 1;
            if self.start_index > self.max_length {
                self.start_index = 0;
            }
        }
        amount_avail
    }

    pub fn write_from(&mut self, amount: usize, buffer: &Vec<i16>) -> usize {
        assert!(amount >= 0);
        let amount_avail = min(min(amount, buffer.len()), self.max_length);
        for _ in self.buffer.len()..min(self.end_index + amount_avail + 1, self.max_length + 1) {
            self.buffer.push(0);
        }
        for i in 0..amount_avail {
            self.buffer[self.end_index] = buffer[i];
            self.end_index += 1;
            if self.end_index > self.max_length {
                self.end_index = 0;
            }
            if self.end_index == self.start_index {
                self.start_index = self.end_index + 1;
                if self.start_index > self.max_length {
                    self.start_index = 0;
                }
            }
        }
        amount_avail
    }

    pub fn write_from_ring(&mut self, amount: usize, ring: &mut RingBuffer) -> usize {
        assert!(amount >= 0);
        let amount_avail = min(min(amount, ring.len()), self.max_length);
        for _ in self.buffer.len()..min(self.end_index + amount_avail + 1, self.max_length + 1) {
            self.buffer.push(0);
        }
        for _ in 0..amount_avail {
            self.buffer[self.end_index] = ring.buffer[ring.start_index];
            self.end_index += 1;
            if self.end_index > self.max_length {
                self.end_index = 0;
            }
            if self.end_index == self.start_index {
                self.start_index = self.end_index + 1;
                if self.start_index > self.max_length {
                    self.start_index = 0;
                }
            }
            ring.start_index += 1;
            if ring.start_index > ring.max_length {
                ring.start_index = 0;
            }
        }
        amount_avail
    }
}

#[cfg(test)]
mod tests {
    use super::RingBuffer;

    #[test]
    fn it_reads_into() {
        let mut a = RingBuffer::new();
        let mut v = Vec::<i16>::new();

        for i in 0..48 {
            a.buffer.push(i as i16);
        }
        a.end_index = 48;

        assert_eq!(v.len(), 0);
        assert_eq!(a.len(), 48);
        assert_eq!(a.read_into(49, &mut v), 48);
        assert_eq!(v.len(), 48);
        assert_eq!(a.len(), 0);
        assert_eq!(v[0], 0);
    }

    #[test]
    fn it_writes_from() {
        let mut a = RingBuffer::new();
        let mut v = Vec::<i16>::new();

        for i in 0..48 {
            v.push(i as i16);
        }

        assert_eq!(v.len(), 48);
        assert_eq!(a.len(), 0);
        assert_eq!(a.write_from(49, &mut v), 48);
        assert_eq!(v.len(), 48);
        assert_eq!(a.len(), 48);
        assert_eq!(a.end_index, 48);
    }

    #[test]
    fn it_writes_from_ring() {
        let mut a = RingBuffer::new();
        let mut v = Vec::<i16>::new();

        for i in 0..48 {
            v.push(i as i16);
        }
        let mut b = RingBuffer::new();
        b.write_from(48, &v);

        assert_eq!(b.len(), 48);
        assert_eq!(a.len(), 0);
        assert_eq!(a.write_from_ring(49, &mut b), 48);
        assert_eq!(b.len(), 0);
        assert_eq!(a.len(), 48);
        assert_eq!(a.end_index, 48);
        assert_eq!(b.start_index, 48);
    }

    #[test]
    fn it_reads_into_maxlen() {
        let mut a = RingBuffer::new();
        let mut v = Vec::<i16>::new();

        for i in 0..49 {
            a.buffer.push(i as i16);
        }
        a.max_length = 48;
        a.start_index = 24;
        a.end_index = 23;

        assert_eq!(v.len(), 0);
        assert_eq!(a.len(), 48);
        assert_eq!(a.read_into(49, &mut v), 48);
        assert_eq!(v.len(), 48);
        assert_eq!(a.len(), 0);
        assert_eq!(v[0], 24);
    }

    #[test]
    fn it_writes_from_maxlen() {
        let mut a = RingBuffer::new();
        let mut v = Vec::<i16>::new();

        for i in 0..48 {
            v.push(i as i16);
        }
        a.max_length = 48;
        a.start_index = 24;
        a.end_index = 24;

        assert_eq!(v.len(), 48);
        assert_eq!(a.len(), 0);
        assert_eq!(a.write_from(49, &mut v), 48);
        assert_eq!(v.len(), 48);
        assert_eq!(a.len(), 48);
        assert_eq!(a.end_index, 23);
    }

    #[test]
    fn it_writes_from_ring_maxlen() {
        let mut a = RingBuffer::new();
        let mut v = Vec::<i16>::new();

        for i in 0..48 {
            v.push(i as i16);
        }
        a.max_length = 48;
        a.start_index = 24;
        a.end_index = 24;
        let mut b = RingBuffer::new();
        b.max_length = 48;
        b.start_index = 12;
        b.end_index = 12;
        b.write_from(48, &v);
        assert_eq!(b.end_index, 11);

        assert_eq!(b.len(), 48);
        assert_eq!(a.len(), 0);
        assert_eq!(a.write_from_ring(49, &mut b), 48);
        assert_eq!(b.len(), 0);
        assert_eq!(a.len(), 48);
        assert_eq!(a.end_index, 23);
        assert_eq!(b.start_index, 11);
    }
}
