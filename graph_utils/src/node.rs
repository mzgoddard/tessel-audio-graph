use std::any::Any;

use super::{RingBuffer};

pub trait NodeAsAny : Any {
    fn as_any(&self) -> &Any;
    fn as_mut_any(&mut self) -> &mut Any;
}

pub trait Node : NodeAsAny {
    fn update(&mut self, inputs: &mut [RingBuffer], outputs: &mut [RingBuffer]);
    // fn is_input(&self) -> bool;
    // fn is_output(&self) -> bool;
    // fn channels(&self) -> usize;
    // fn available(&self) -> usize;
    // fn read_into(&mut self, samples: usize, buffer: &mut Vec<i16>);
}

impl<T> NodeAsAny for T where T : Any {
  fn as_any(&self) -> &Any {
    self as &Any
  }
  fn as_mut_any(&mut self) -> &mut Any {
      self as &mut Any
  }
}

impl Node {
    pub fn downcast_ref<T: Any>(&self) -> Option<&T> {
        self.as_any().downcast_ref::<T>()
    }

    pub fn downcast_mut<T: Any>(&mut self) -> Option<&mut T> {
        self.as_mut_any().downcast_mut::<T>()
    }
}

// #[cfg(test)]
// mod test {
// }
