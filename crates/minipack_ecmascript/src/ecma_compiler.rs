use arcstr::ArcStr;
use minipack_error::BuildResult;
use oxc::{
  allocator::Allocator,
  codegen::{CodeGenerator, Codegen, CodegenOptions, CodegenReturn, LegalComment},
  minifier::{CompressOptions, CompressOptionsKeepNames, MangleOptions, Minifier, MinifierOptions},
  parser::{ParseOptions, Parser},
  span::SourceType,
  transformer::ESTarget,
};

use crate::ecma_ast::{
  EcmaAst,
  program_cell::{ProgramCell, ProgramCellDependent, ProgramCellOwner},
};

pub struct EcmaCompiler;

impl EcmaCompiler {
  pub fn parse(source: impl Into<ArcStr>, source_type: SourceType) -> BuildResult<EcmaAst> {
    let source: ArcStr = source.into();
    let allocator = oxc::allocator::Allocator::default();
    let program = ProgramCell::try_new(ProgramCellOwner { source, allocator }, |owner| {
      let parser =
        Parser::new(&owner.allocator, &owner.source, source_type).with_options(ParseOptions {
          allow_return_outside_function: true,
          ..ParseOptions::default()
        });
      let ret = parser.parse();
      if ret.panicked || !ret.errors.is_empty() {
        Err(anyhow::anyhow!("{:?}", ret.errors))
      } else {
        Ok(ProgramCellDependent { program: ret.program })
      }
    })?;

    Ok(EcmaAst { program, source_type, contains_use_strict: false })
  }

  pub fn print(ast: &EcmaAst) -> CodegenReturn {
    CodeGenerator::new()
      .with_options(CodegenOptions {
        comments: true,
        legal_comments: LegalComment::Inline,
        ..CodegenOptions::default()
      })
      .build(ast.program())
  }

  pub fn minify(source_text: &str, target: ESTarget) -> String {
    let allocator = Allocator::default();
    let source_type = SourceType::default();

    let program = Parser::new(&allocator, source_text, source_type).parse().program;
    let program = allocator.alloc(program);

    let ret = Minifier::new(MinifierOptions {
      mangle: Some(MangleOptions::default()),
      compress: Some(CompressOptions {
        target,
        drop_debugger: false,
        drop_console: false,
        keep_names: CompressOptionsKeepNames { function: true, class: true },
      }),
    })
    .build(&allocator, program);

    let ret = Codegen::new()
      .with_options(CodegenOptions { minify: true, ..CodegenOptions::default() })
      .with_scoping(ret.scoping)
      .build(program);

    ret.code
  }
}

#[test]
fn basic_test() {
  let ast = EcmaCompiler::parse("const a = 1;".to_string(), SourceType::default()).unwrap();
  let code = EcmaCompiler::print(&ast).code;
  assert_eq!(code, "const a = 1;\n");
}
