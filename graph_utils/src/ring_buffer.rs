use std::cmp::{min, max};
use std::ops::{Index, IndexMut, Range};
use std::iter::{Chain, Iterator};
use std::slice;

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

    pub fn read_slice<'a>(&'a mut self, amount: usize) -> RingSlice<'a> {
        let avail = min(amount, self.len());
        let start_index = self.start_index;

        self._bump_start(avail);

        RingSlice {
            _len: avail,
            start: start_index,
            ring: self,
        }
    }

    pub fn read_into(&mut self, amount: usize, buffer: &mut Vec<i16>) -> usize {
        assert!(amount >= 0);
        let amount_avail = min(amount, self.len());
        for _ in (buffer.len())..amount_avail {
            buffer.push(0);
        }
        for (o, i) in buffer.iter_mut().zip(self.read_slice(amount_avail).iter()) {
            *o = *i;
        }
        // for (i, j) in (0..amount_avail).zip(self._start_range_iter(amount_avail)) {
        //     buffer[i] = self.buffer[j];
        // }
        // self._bump_start(amount_avail);
        amount_avail
    }

    pub fn write_slice<'a>(&'a mut self, amount: usize) -> RingSlice<'a> {
        let avail = min(amount, self.max_length);
        let end_index = self.end_index;

        self._bump_end(avail);

        for _ in self.buffer.len()..min(end_index + avail + 1, self.max_length + 1) {
            self.buffer.push(0);
        }

        RingSlice {
            _len: avail,
            start: end_index,
            ring: self,
        }
    }

    fn _write_from<'a, T>(&mut self, amount: usize, buffer: T) -> usize where T : Iterator<Item=&'a i16> {
        assert!(amount >= 0);
        let amount_avail = min(amount, self.max_length);
        // for _ in self.buffer.len()..min(self.end_index + amount_avail + 1, self.max_length + 1) {
        //     self.buffer.push(0);
        // }
        // for (i, j) in (0..amount_avail).zip(self._end_range_iter(amount_avail)) {
        //     self.buffer[j] = buffer[i];
        // }
        for (i, o) in buffer.zip(self.write_slice(amount_avail).iter_mut()) {
            *o = *i;
        }

        // self._bump_end(amount_avail);

        amount_avail
    }

    pub fn write_from(&mut self, amount: usize, buffer: &Vec<i16>) -> usize {
        self._write_from(min(amount, buffer.len()), buffer.iter())
    }

    pub fn write_from_read_slice(&mut self, amount: usize, buffer: &RingSlice) -> usize {
        self._write_from(min(amount, buffer.len()), buffer.iter())
    }

    fn _range_iter(start: usize, len: usize, max_len: usize) -> Chain<Range<usize>, Range<usize>> {
        (start..min(start + len, max_len + 1))
        .chain(0..max((start + len) as i32 - (max_len + 1) as i32, 0) as usize)
    }

    fn _start_range_iter(&self, len: usize) -> Chain<Range<usize>, Range<usize>> {
        RingBuffer::_range_iter(self.start_index, len, self.max_length)
    }

    fn _end_range_iter(&self, len: usize) -> Chain<Range<usize>, Range<usize>> {
        RingBuffer::_range_iter(self.end_index, len, self.max_length)
    }

    fn _bump_start(&mut self, len: usize) {
        self.start_index += len;
        if self.start_index > self.max_length {
            self.start_index -= self.max_length + 1;
        }
    }

    fn _bump_end(&mut self, len: usize) {
        if self.end_index < self.start_index && self.end_index + len >= self.start_index {
            self.start_index = self.end_index + len + 1;
            if self.start_index > self.max_length {
                self.start_index -= self.max_length - 1;
            }
        }
        else if self.end_index > self.start_index &&
            self.end_index + len > self.max_length &&
            self.end_index + len - self.max_length >= self.start_index
        {
            self.start_index = self.end_index + len - self.max_length + 1;
        }
        self.end_index += len;
        if self.end_index > self.max_length {
            self.end_index -= self.max_length + 1;
        }
    }

    pub fn write_from_ring(&mut self, amount: usize, ring: &mut RingBuffer) -> usize {
        assert!(amount >= 0);
        let amount_avail = min(min(amount, ring.len()), self.max_length);
        // for _ in self.buffer.len()..min(self.end_index + amount_avail + 1, self.max_length + 1) {
        //     self.buffer.push(0);
        // }
        // for (i, j) in ring._start_range_iter(amount_avail).zip(self._end_range_iter(amount_avail)) {
        //     self.buffer[j] = ring.buffer[i];
        // }
        for (i, o) in ring.read_slice(amount_avail).iter().zip(self.write_slice(amount_avail).iter_mut()) {
            *o = *i;
        }

        // self._bump_end(amount_avail);
        // ring._bump_start(amount_avail);

        amount_avail
    }
}

pub struct RingSlice<'a> {
    _len: usize,
    start: usize,
    ring: &'a mut RingBuffer,
}

// pub struct RingSliceIter<'a> {
//     slice: &'a RingSlice,
//     index_iter: Range<usize>,
// }
//
// impl<'a> Iterator for RingSliceIter<'a> {
//     type Item = &i16;
//     fn next(&mut self) -> Option<Self::Item> {
//         match self.index_iter.next() {
//             Some(index) => {
//                 Some(self.slice.ring.buffer[index])
//             },
//             None => None,
//         }
//     }
// }
//
// pub struct RingSliceIterMut<'a> {
//     slice: &'a mut RingSlice,
//     index_iter: Range<usize>,
// }
//
// impl<'a> Iterator for RingSliceIterMut<'a> {
//     type Item = &mut i16;
//     fn next(&mut self) -> Option<Self::Item> {
//         match self.index_iter.next() {
//             Some(index) => {
//                 Some(self.slice.ring.buffer[index])
//             },
//             None => None,
//         }
//     }
// }

impl<'a> RingSlice<'a> {
    pub fn len(&self) -> usize {
        self._len
    }

    pub fn iter<'b>(&'b self) -> Chain<slice::Iter<'b, i16>, slice::Iter<'b, i16>> {
        let start = self.start;
        let len = self._len;
        let max_len = self.ring.max_length;

        self.ring.buffer[start..min(start + len, max_len + 1)].iter()
        .chain(self.ring.buffer[0..max((start + len) as i32 - (max_len + 1) as i32, 0) as usize].iter())
    }

    pub fn iter_mut<'b>(&'b mut self) -> Chain<slice::IterMut<'b, i16>, slice::IterMut<'b, i16>> {
        let start = self.start;
        let len = self._len;
        let max_len = self.ring.max_length;

        unsafe {
            let buffer_ptr = self.ring.buffer.as_mut_ptr();
            slice::from_raw_parts_mut(buffer_ptr.offset(start as isize), min(start + len, max_len + 1) - start).iter_mut()
            .chain(slice::from_raw_parts_mut(buffer_ptr, max((start + len) as i32 - (max_len + 1) as i32, 0) as usize).iter_mut())
        }
    }
}

impl<'a> Index<usize> for RingSlice<'a> {
    type Output = i16;
    fn index(&self, mut index: usize) -> &Self::Output {
        assert!(index < self._len);
        index += self.start;
        if index > self.ring.max_length {
            index -= self.ring.max_length + 1;
        }
        &self.ring.buffer[index]
    }
}

impl<'a> IndexMut<usize> for RingSlice<'a> {
    fn index_mut(&mut self, mut index: usize) -> &mut Self::Output {
        index += self.start;
        if index > self.ring.max_length {
            index -= self.ring.max_length + 1;
        }
        &mut self.ring.buffer[index]
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

    #[test]
    fn it_iters_slices() {
        let mut a = RingBuffer::new();
        a.max_length = 48;
        let mut v = Vec::<i16>::new();

        for i in 0..48 {
            v.push(i as i16);
        }

        assert_eq!(a.write_from(48, &mut v), 48);
        for (i, v) in a.read_slice(48).iter().enumerate() {
            assert_eq!(*v, i as i16);
        }

        for (i, v) in a.write_slice(48).iter_mut().enumerate() {
            *v = i as i16 + 1;
        }

        for (i, v) in a.read_slice(48).iter().enumerate() {
            assert_eq!(*v, i as i16 + 1);
        }

        assert_eq!(a.write_slice(49).len(), 48);
        assert_eq!(a.read_slice(49).len(), 48);
        assert_eq!(a.write_slice(49).iter().count(), 48);
        assert_eq!(a.read_slice(49).iter().count(), 48);
        assert_eq!(a.write_slice(48).iter().count(), 48);
        assert_eq!(a.read_slice(48).iter().count(), 48);
        assert_eq!(a.write_slice(47).iter().count(), 47);
        assert_eq!(a.read_slice(47).iter().count(), 47);
        assert_eq!(a.write_slice(49).iter_mut().count(), 48);
        assert_eq!(a.read_slice(49).iter_mut().count(), 48);
        assert_eq!(a.write_slice(48).iter_mut().count(), 48);
        assert_eq!(a.read_slice(48).iter_mut().count(), 48);
        assert_eq!(a.write_slice(47).iter_mut().count(), 47);
        assert_eq!(a.read_slice(47).iter_mut().count(), 47);
    }
}
