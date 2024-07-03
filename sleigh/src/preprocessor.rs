use std::collections::HashMap;
use std::env::current_dir;
use std::fs;
use std::path::{
    Component,
    PathBuf,
};
use std::process::exit;

use ariadne::{
    Label,
    Report,
    ReportKind,
    Source,
};
use chumsky::container::{
    OrderedSeq,
    Seq,
};
use chumsky::error::{
    Rich,
    Simple,
};
use chumsky::input::Input;
use chumsky::prelude::{
    any,
    choice,
    end,
    just,
    none_of,
    recursive,
    todo,
};
use chumsky::recursive::Recursive;
use chumsky::text::{
    ident,
    inline_whitespace,
};
use chumsky::{
    extra,
    IterParser,
    Parser,
};
use tracing::trace;

use crate::preprocessor::error::PreprocessorError;

pub mod error;

#[derive(Debug, PartialEq, Eq, Hash, Clone)]
pub struct PreprocessorIdentifier(String);

impl From<PreprocessorIdentifier> for String {
    fn from(value: PreprocessorIdentifier) -> Self {
        value.0
    }
}

#[derive(Debug, Clone)]
enum BinaryOp {
    And,
    Or,
    Xor,
}

#[derive(Debug, Clone)]
enum ComparisonArgument {
    Identifier(PreprocessorIdentifier),
    Literal(String),
}

#[derive(Debug, Clone)]
enum EqualityCheck {
    Eq(ComparisonArgument, ComparisonArgument),
    Ne(ComparisonArgument, ComparisonArgument),
}

#[derive(Debug, Clone)]
enum Expression {
    BinaryOp(Box<Self>, BinaryOp, Box<Self>),
    EqualityCheck(EqualityCheck),
    Defined(PreprocessorIdentifier),
}

#[derive(Debug, Clone)]
enum PreprocessorBlock {
    Define {
        name: PreprocessorIdentifier,
        value: String,
    },
    Undefine(PreprocessorIdentifier),
    Include(PathBuf),
    IfDef {
        ident: PreprocessorIdentifier,
        if_defined: Option<Vec<Self>>,
        if_undefined: Option<Vec<Self>>,
    },
    If {
        cond: Box<Expression>,
        if_true: Vec<Self>,
        if_false: Option<Vec<Self>>,
    },
    PCode(String),
    Expansion(PreprocessorIdentifier),
}

#[derive(Default, Debug)]
pub struct SleighPreprocessor<'src> {
    working_dir: PathBuf,
    original_data: &'src str,
    included_data: Vec<String>,
    processed_sleigh: String,
    definitions: HashMap<PreprocessorIdentifier, String>,
}

impl<'a> SleighPreprocessor<'a> {
    pub fn preprocess(
        mut self,
        spec_data: &'a str,
    ) -> Result<String, PreprocessorError> {
        self.original_data = spec_data;
        let parsed = self.parse(self.original_data)?;
        self.evaluate(parsed)?;
        println!("{}", self.processed_sleigh);
        Ok(self.processed_sleigh)
    }

    fn parse(
        &self,
        data: &str,
    ) -> Result<Vec<PreprocessorBlock>, PreprocessorError> {
        let parsed = parser().parse(data);
        let mut diagnostic = vec![];
        parsed.errors().for_each(|err| {
            let report = Report::build(ReportKind::Error, (), err.span().start)
                .with_message("Parsing Error")
                .with_label(Label::new(err.span().into_range()))
                .with_message(err.reason().to_string())
                .finish();
            report.write(Source::from(data), &mut diagnostic).unwrap();
        });
        if diagnostic.is_empty() {
            Ok(parsed.into_output().unwrap())
        } else {
            println!("{}", std::str::from_utf8(diagnostic.as_slice()).unwrap());
            Err(PreprocessorError::ParseError(String::from_utf8(diagnostic).unwrap()))
        }
    }

    fn evaluate(
        &mut self,
        blocks: Vec<PreprocessorBlock>,
    ) -> Result<(), PreprocessorError> {
        for block in blocks {
            match block {
                PreprocessorBlock::Define { name, value } => {
                    trace!("Defining {name:?} as {value:?}");
                    self.definitions.insert(name, value.clone());
                }
                PreprocessorBlock::Undefine(name) => {
                    trace!("Removed definition for {name:?}");
                    self.definitions.remove(&name);
                }
                PreprocessorBlock::Include(file) => {
                    trace!("Including file {file:?}");
                    if file.is_absolute() {
                        Err(PreprocessorError::AbsolutePath)?;
                    }
                    let mut resolved_file = self.working_dir.clone();
                    for part in file.components() {
                        match part {
                            Component::Prefix(_) => {}
                            Component::RootDir => {}
                            Component::CurDir => {}
                            Component::ParentDir => {}
                            Component::Normal(path) => resolved_file.push(path),
                        }
                    }
                    let data = fs::read_to_string(resolved_file.as_path())?;
                    let blocks = self.parse(&data)?;
                    self.evaluate(blocks)?;
                }
                PreprocessorBlock::IfDef {
                    ident,
                    if_defined,
                    if_undefined,
                } => {
                    trace!("Processing ifdef {ident:?}");
                    if self.definitions.contains_key(&ident) {
                        trace!("{ident:?} is defined");
                        match if_defined {
                            None => {}
                            Some(blocks) => self.evaluate(blocks)?,
                        }
                    } else {
                        trace!("{ident:?} is undefined");
                        match if_undefined {
                            None => {}
                            Some(blocks) => self.evaluate(blocks)?,
                        };
                    }
                }
                PreprocessorBlock::PCode(code) => {
                    self.processed_sleigh.push_str(&code);
                }
                PreprocessorBlock::Expansion(ident) => {
                    let value = self
                        .definitions
                        .get(&ident)
                        .ok_or(PreprocessorError::UndefinedIdentifier(ident.clone().into()))?;
                    trace!("Expanding macro {ident:?} to {value:?}");
                    self.processed_sleigh.push_str(value)
                }
                PreprocessorBlock::If {
                    cond,
                    if_true,
                    if_false,
                } => {
                    trace!("Evaluating condition {cond:?}");
                    if self.evaluate_expression(cond)? {
                        trace!("Evaluated to true");
                        self.evaluate(if_true)?
                    } else {
                        trace!("Evaluated to false");
                        match if_false {
                            None => {}
                            Some(blocks) => self.evaluate(blocks)?,
                        }
                    }
                }
            };
        }
        Ok(())
    }

    fn evaluate_expression(
        &self,
        expression: Box<Expression>,
    ) -> Result<bool, PreprocessorError> {
        Ok(match *expression {
            Expression::BinaryOp(a, op, b) => match op {
                BinaryOp::And => self.evaluate_expression(a)? && self.evaluate_expression(b)?,
                BinaryOp::Or => self.evaluate_expression(a)? || self.evaluate_expression(b)?,
                BinaryOp::Xor => self.evaluate_expression(a)? ^ self.evaluate_expression(b)?,
            },
            Expression::Defined(ident) => self.definitions.get(&ident).map(|_| true).unwrap_or(false),
            Expression::EqualityCheck(check) => match check {
                EqualityCheck::Eq(a, b) => {
                    self.resolve_comparison_argument(&a)? == self.resolve_comparison_argument(&b)?
                }
                EqualityCheck::Ne(a, b) => {
                    self.resolve_comparison_argument(&a)? != self.resolve_comparison_argument(&b)?
                }
            },
        })
    }

    fn resolve_comparison_argument<'arg: 'a>(
        &self,
        arg: &'arg ComparisonArgument,
    ) -> Result<&String, PreprocessorError> {
        Ok(match arg {
            ComparisonArgument::Identifier(ident) => self
                .definitions
                .get(&ident)
                .ok_or(PreprocessorError::UndefinedIdentifier(ident.clone().into()))?,
            ComparisonArgument::Literal(literal) => literal,
        })
    }
}

fn parser<'src>() -> impl Parser<'src, &'src str, Vec<PreprocessorBlock>, extra::Err<Rich<'src, char>>> {
    let mut blocks = Recursive::declare();

    let reserved = choice((just("@"), just("$(")));

    let pcode = any()
        .and_is(reserved.not())
        .repeated()
        .at_least(1)
        .to_slice()
        .map(|p| PreprocessorBlock::PCode(String::from(p)));

    let string = just('"')
        .ignore_then(none_of('"').repeated().to_slice())
        .then_ignore(just('"'))
        .map(String::from);

    let preprocessor_ident = ident().map(|id| PreprocessorIdentifier(String::from(id)));

    let include = just("@include")
        .ignore_then(string.padded_by(inline_whitespace()))
        .map(|file| PreprocessorBlock::Include(PathBuf::from(file)));

    let define = just("@define")
        .ignore_then(ident().padded_by(inline_whitespace()))
        .then(string)
        .padded_by(inline_whitespace())
        .map(|(name, value)| PreprocessorBlock::Define {
            name: PreprocessorIdentifier(String::from(name)),
            value,
        })
        .boxed();

    let undef = just("@undef")
        .ignore_then(ident().padded_by(inline_whitespace()))
        .map(|ident| PreprocessorBlock::Undefine(PreprocessorIdentifier(String::from(ident))));

    let expansion = just('$')
        .ignore_then(ident().to_slice().delimited_by(just('('), just(')')))
        .map(|ident| PreprocessorBlock::Expansion(PreprocessorIdentifier(String::from(ident))));

    let r#else = just("@else")
        .ignore_then(blocks.clone())
        .then_ignore(just("@endif"));

    let ifdef = just("@ifdef")
        .ignore_then(preprocessor_ident.padded_by(inline_whitespace()))
        .then(blocks.clone().map(Some))
        .then(choice((r#else.clone().map(Some), just("@endif").map(|_| None))))
        .map(|((ident, if_defined), if_undefined)| PreprocessorBlock::IfDef {
            ident,
            if_defined,
            if_undefined,
        })
        .boxed();

    let ifndef = just("@ifndef")
        .ignore_then(preprocessor_ident.padded_by(inline_whitespace()))
        .then(blocks.clone().map(Some))
        .then(choice((r#else.clone().map(Some), just("@endif").map(|_| None))))
        .map(|((ident, if_undefined), if_defined)| PreprocessorBlock::IfDef {
            ident,
            if_defined,
            if_undefined,
        })
        .boxed();

    let defined = just("defined")
        .ignore_then(preprocessor_ident.delimited_by(just('('), just(')')))
        .map(|expr| Box::new(Expression::Defined(expr)))
        .padded_by(inline_whitespace());

    let comparison_argument = preprocessor_ident
        .map(ComparisonArgument::Identifier)
        .or(string.map(ComparisonArgument::Literal))
        .padded_by(inline_whitespace());

    let equality_check = comparison_argument
        .then(just("==").or(just("!=")))
        .then(comparison_argument)
        .map(|((a, op), b)| match op {
            "==" => EqualityCheck::Eq(a, b),
            "!=" => EqualityCheck::Ne(a, b),
            _ => panic!("This cannot occur"),
        })
        .map(|check| Box::new(Expression::EqualityCheck(check)));

    let mut expression = Recursive::declare();

    let atom = defined
        .or(equality_check.or(equality_check.delimited_by(just('('), just(')'))))
        .or(expression.clone().delimited_by(just('('), just(')')))
        .padded_by(inline_whitespace());

    let operator = choice((just("&&"), just("||"), just("^^")))
        .map(|op| match op {
            "&&" => BinaryOp::And,
            "||" => BinaryOp::Or,
            "^^" => BinaryOp::Xor,
            _ => panic!("This cannot occur"),
        })
        .padded_by(inline_whitespace());

    let binary_op = atom
        .clone()
        .foldl(operator.then(atom).repeated(), |a, (op, b)| Box::new(Expression::BinaryOp(a, op, b)));

    expression.define(binary_op);

    let r#if = just("@if")
        .ignore_then(expression)
        .then(blocks.clone())
        .then(choice((r#else.map(Some), just("@endif").map(|_| None))))
        .map(|((cond, if_true), if_false)| PreprocessorBlock::If {
            cond,
            if_true,
            if_false,
        })
        .boxed();

    blocks.define(
        choice((define, undef, include, ifdef, ifndef, r#if, expansion, pcode))
            .repeated()
            .at_least(1)
            .collect::<Vec<_>>()
            .or(end().map(|_| Vec::new())),
    );
    blocks
}

#[derive(Default, Debug)]
pub struct SleighPreprocessorFactory {
    working_dir: Option<PathBuf>,
}

impl SleighPreprocessorFactory {
    pub fn new() -> Self {
        Self { working_dir: None }
    }

    pub fn working_dir(
        mut self,
        dir: PathBuf,
    ) -> Self {
        self.working_dir = Some(dir);
        self
    }

    pub fn build(&self) -> SleighPreprocessor {
        let mut preprocessor = SleighPreprocessor::default();
        preprocessor.working_dir = self
            .working_dir
            .clone()
            .unwrap_or_else(|| current_dir().unwrap());
        preprocessor
    }
}
