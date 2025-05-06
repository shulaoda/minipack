use arcstr::ArcStr;
use minipack_error::BuildResult;
use oxc::{
  allocator::Allocator,
  codegen::{Codegen, CodegenOptions, CodegenReturn},
  minifier::{CompressOptions, CompressOptionsKeepNames, MangleOptions, Minifier, MinifierOptions},
  parser::Parser,
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
    let allocator = oxc::allocator::Allocator::default();
    let owner = ProgramCellOwner { source: source.into(), allocator };
    let program = ProgramCell::try_new(owner, |owner| {
      let ret = Parser::new(&owner.allocator, &owner.source, source_type).parse();
      if ret.errors.is_empty() {
        Ok(ProgramCellDependent { program: ret.program })
      } else {
        Err(anyhow::anyhow!("{:?}", ret.errors))
      }
    })?;

    Ok(EcmaAst { program, source_type })
  }

  pub fn print(ast: &EcmaAst) -> CodegenReturn {
    Codegen::new().build(ast.program())
  }

  pub fn minify(source_text: &str) -> String {
    let allocator = Allocator::default();
    let source_type = SourceType::default();

    let program = Parser::new(&allocator, source_text, source_type).parse().program;
    let program = allocator.alloc(program);

    let ret = Minifier::new(MinifierOptions {
      mangle: Some(MangleOptions::default()),
      compress: Some(CompressOptions {
        target: ESTarget::ESNext,
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
