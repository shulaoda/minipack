pub use rayon::iter::{
  IndexedParallelIterator, IntoParallelIterator, IntoParallelRefIterator,
  IntoParallelRefMutIterator, ParallelBridge, ParallelIterator,
};

fn _usages() {
  let mut demo = vec![1, 2, 3, 4, 5];
  demo.iter().par_bridge().for_each(|_| {});
  demo.iter_mut().par_bridge().for_each(|_| {});
  demo.clone().into_iter().par_bridge().for_each(|_| {});
  demo.par_iter().for_each(|_| {});
  // demo.par_iter_mut().for_each(|_| {});
  demo.clone().into_par_iter().for_each(|_| {});
}
