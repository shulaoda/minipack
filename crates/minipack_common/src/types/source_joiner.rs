use super::source::Source;

#[derive(Default)]
pub struct SourceJoiner<'source> {
  inner: Vec<Box<dyn Source + Send + 'source>>,
  prepend_source: Vec<Box<dyn Source + Send + 'source>>,
}

impl<'source> SourceJoiner<'source> {
  pub fn append_source<T: Source + Send + 'source>(&mut self, source: T) {
    self.inner.push(Box::new(source));
  }

  pub fn prepend_source(&mut self, source: Box<dyn Source + Send + 'source>) {
    self.prepend_source.push(source);
  }

  pub fn join(&self) -> String {
    let sources_len = self.prepend_source.len() + self.inner.len();
    let sources_iter = self.prepend_source.iter().chain(self.inner.iter()).enumerate();

    let size_hint_of_ret_source = sources_iter.clone().map(|(_idx, source)| source.content().len()).sum::<usize>()
        + /* Each source we will emit a '\n' but exclude last one */ (sources_len - /* Exclude the last source  */ 1);
    let mut ret_source = String::with_capacity(size_hint_of_ret_source);

    for (index, source) in sources_iter {
      ret_source.push_str(source.content());
      if index < sources_len - 1 {
        ret_source.push('\n');
      }
    }

    ret_source
  }
}
