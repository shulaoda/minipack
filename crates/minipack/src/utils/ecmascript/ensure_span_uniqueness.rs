use oxc::span::{SPAN, Span};
use rustc_hash::FxHashSet;

/// Make sure there aren't any duplicate spans in the AST.
pub struct EnsureSpanUniqueness {
  visited_spans: FxHashSet<Span>,
  pub next_unique_span_start: u32,
}

impl EnsureSpanUniqueness {
  pub fn new() -> Self {
    Self { visited_spans: FxHashSet::from_iter([SPAN]), next_unique_span_start: 1 }
  }

  pub fn generate_unique_span(&mut self) -> Span {
    let mut span_candidate = Span::new(self.next_unique_span_start, self.next_unique_span_start);
    while self.visited_spans.contains(&span_candidate) {
      self.next_unique_span_start += 1;
      span_candidate = Span::new(self.next_unique_span_start, self.next_unique_span_start);
    }
    self.visited_spans.insert(span_candidate);
    span_candidate
  }
}
