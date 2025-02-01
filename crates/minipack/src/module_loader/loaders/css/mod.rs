pub mod css_generator;

use arcstr::ArcStr;

use minipack_common::{
  CssRenderer, CssView, ImportKind, ImportRecordIdx, RawImportRecord, SymbolRef,
};
use oxc::span::Span;
use oxc_index::IndexVec;

pub fn create_css_view(
  source: impl Into<ArcStr>,
) -> (CssView, IndexVec<ImportRecordIdx, RawImportRecord>, Vec<anyhow::Error>) {
  let source: ArcStr = source.into();
  let (lexed_deps, warnings) =
    css_module_lexer::collect_dependencies(&source, css_module_lexer::Mode::Css);

  let mut css_renderer = CssRenderer::default();
  let mut dependencies = IndexVec::default();
  let mut record_idx_to_span = IndexVec::default();

  for lexed_dep in lexed_deps {
    match lexed_dep {
      css_module_lexer::Dependency::Import { request, range, .. } => {
        dependencies.push(RawImportRecord::new(
          request.into(),
          ImportKind::AtImport,
          SymbolRef::default(),
          Span::new(range.start, range.end),
          None,
        ));

        record_idx_to_span.push(Span::new(range.start, range.end));

        let mut range_end = range.end as usize;
        if source.is_char_boundary(range_end) {
          if source[range_end..].starts_with("\r\n") {
            range_end += 2;
          }
          if source[range_end..].starts_with('\n') {
            range_end += 1;
          }
        }

        css_renderer.at_import_ranges.push((range.start as usize, range_end));
      }
      css_module_lexer::Dependency::Url { request, range, kind } => {
        // css_module_lexer return span of `request` if kind is `string`, return whole span of `url(dep)`, if the kind is function
        // so we need to tweak a little to get the correct span we want that used to replace
        // asset filename
        let span = if matches!(kind, css_module_lexer::UrlRangeKind::String) {
          Span::new(range.start + 1, range.end - 1)
        } else {
          Span::new(range.start + 4 /*length of `url(`*/, range.end - 1)
        };

        dependencies.push(RawImportRecord::new(
          request.into(),
          ImportKind::UrlImport,
          SymbolRef::default(),
          span,
          None,
        ));
        record_idx_to_span.push(span);
      }
      _ => {}
    }
  }

  let warnings =
    warnings.into_iter().map(|warning| anyhow::anyhow!("{warning}")).collect::<Vec<_>>();

  (
    CssView {
      source,
      import_records: IndexVec::default(),
      mutations: vec![Box::new(css_renderer)],
      record_idx_to_span,
    },
    dependencies,
    warnings,
  )
}
