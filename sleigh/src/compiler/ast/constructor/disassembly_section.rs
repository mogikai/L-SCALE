use crate::compiler::ast::Spanned;
use crate::compiler::ast::common::SASTIdent;

/// Used inside the disassembly section for arbitrary calculations
#[derive(Debug, Clone)]
pub enum SASTPatternExpression {
    Add(Box<Spanned<SASTPatternExpression>>, Box<Spanned<SASTPatternExpression>>),
    Sub(Box<Spanned<SASTPatternExpression>>, Box<Spanned<SASTPatternExpression>>),
    Mul(Box<Spanned<SASTPatternExpression>>, Box<Spanned<SASTPatternExpression>>),
    Div(Box<Spanned<SASTPatternExpression>>, Box<Spanned<SASTPatternExpression>>),
    Shl(Box<Spanned<SASTPatternExpression>>, Box<Spanned<SASTPatternExpression>>),
    Shr(Box<Spanned<SASTPatternExpression>>, Box<Spanned<SASTPatternExpression>>), // arithmetic right-shift
    And(Box<Spanned<SASTPatternExpression>>, Box<Spanned<SASTPatternExpression>>),
    Or(Box<Spanned<SASTPatternExpression>>, Box<Spanned<SASTPatternExpression>>),
    Xor(Box<Spanned<SASTPatternExpression>>, Box<Spanned<SASTPatternExpression>>),
    Not(Box<Spanned<SASTPatternExpression>>),
    Symbol(SASTIdent),
    Integer(i64), // integer constants are treated as signed
}

/// A disassembly action is of the form `<IDENT> = <EXPRESSION>;`
#[derive(Debug)]
pub struct SASTAction {
    pub lhs: Spanned<SASTIdent>,
    pub rhs: Spanned<SASTPatternExpression>,
}

/// The disassembly section allows arbitrary dynamic calculations through the use of pattern
/// expressions. A canonical example is the calculation of an absolute location for a branching
/// instruction from the corresponding relative location
#[derive(Debug)]
pub struct SASTDisassemblySection {
    pub actions: Option<Vec<Spanned<SASTAction>>>,
}
