use arcstr::ArcStr;
use minipack_error::BuildResult;
use oxc::{
  allocator::Allocator,
  ast::AstBuilder,
  codegen::{CodeGenerator, Codegen, CodegenOptions, CodegenReturn, LegalComment},
  minifier::{Minifier, MinifierOptions},
  parser::{ParseOptions, Parser},
  span::{SourceType, SPAN},
};

use crate::ecma_ast::{
  program_cell::{ProgramCell, ProgramCellDependent, ProgramCellOwner},
  EcmaAst,
};

pub struct EcmaCompiler;

impl EcmaCompiler {
  pub fn parse(source: impl Into<ArcStr>, source_type: SourceType) -> BuildResult<EcmaAst> {
    let source: ArcStr = source.into();
    let allocator = oxc::allocator::Allocator::default();
    let program =
      ProgramCell::try_new(ProgramCellOwner { source: source.clone(), allocator }, |owner| {
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

  pub fn parse_expr_as_program(
    source: impl Into<ArcStr>,
    source_type: SourceType,
  ) -> anyhow::Result<EcmaAst> {
    let source: ArcStr = source.into();
    let allocator = oxc::allocator::Allocator::default();
    let program =
      ProgramCell::try_new(ProgramCellOwner { source: source.clone(), allocator }, |owner| {
        let parser = Parser::new(&owner.allocator, &owner.source, source_type);
        let ret = parser.parse_expression();
        match ret {
          Ok(expr) => {
            let builder = AstBuilder::new(&owner.allocator);
            let program = builder.program(
              SPAN,
              SourceType::default().with_module(true),
              owner.source.as_str(),
              builder.vec(),
              None,
              builder.vec(),
              builder.vec1(builder.statement_expression(SPAN, expr)),
            );
            Ok(ProgramCellDependent { program })
          }
          Err(errors) => Err(anyhow::anyhow!("{:?}", errors)),
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

  pub fn minify(source_text: &str) -> String {
    let allocator = Allocator::default();
    let source_type = SourceType::default();

    let program =
      allocator.alloc(Parser::new(&allocator, source_text, source_type).parse().program);
    let mangler = Minifier::new(MinifierOptions::default()).build(&allocator, program).mangler;
    let ret = Codegen::new()
      .with_options(CodegenOptions { minify: true, ..CodegenOptions::default() })
      .with_mangler(mangler)
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
