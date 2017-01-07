// use std::rc::*;
//
// pub struct ParamRW<T> {
//     value: Rc<T>,
// }
//
// impl<T> ParamRW<T> where T : Copy {
//     pub fn new(t: T) -> ParamRW<T> {
//         ParamRW {
//             value: Rc::new(t),
//         }
//     }
//
//     pub fn get(&self) -> T {
//         *self.value
//     }
//
//     pub fn set(&mut self, t: T) {
//         if let Some(value) = Rc::get_mut(&mut self.value) {
//             *value = t;
//         }
//         else {
//             panic!("Cannot get mutable reference to ParamRW");
//         }
//     }
//
//     pub fn clone_read(&self) -> ParamRead<T> {
//         ParamRead {
//             value: Rc::downgrade(&self.value),
//         }
//     }
// }
//
// pub struct ParamRead<T> {
//     value: Weak<T>,
// }
//
// impl<T> ParamRead<T> where T : Copy {
//     pub fn get(&self) -> Option<T> {
//         if let Some(value) = self.value.upgrade() {
//             Some(*value)
//         }
//         else {
//             None
//         }
//     }
// }
//
// #[cfg(test)]
// mod test {
//     use super::*;
//
//     #[test]
//     fn it_reads() {
//         let p = ParamRW::new(1 as i32);
//         assert_eq!(p.get(), 1);
//     }
//
//     #[test]
//     fn it_writes() {
//         let mut p = ParamRW::new(1 as i32);
//         assert_eq!(p.get(), 1);
//         p.set(2);
//         assert_eq!(p.get(), 2);
//     }
//
//     #[test]
//     fn it_reads_clones() {
//         let mut p = ParamRW::new(1 as i32);
//         assert_eq!(p.get(), 1);
//         let q = p.clone_read();
//         assert_eq!(q.get().unwrap(), 1);
//         p.set(2);
//         assert_eq!(p.get(), 2);
//         assert_eq!(q.get().unwrap(), 2);
//     }
// }
