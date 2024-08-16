use crate::compiler::ast::constructor::bit_pattern_section::SASTBitPatternConstraint;
use crate::compiler::ast::constructor::SASTConstructor;
use crate::compiler::ast::Spanned;
use crate::compiler::{
    CompilerContext,
    SleighCompiler,
    SleighCompilerResult,
};
use crate::spec::moir::constructor::MOIRConstructor;
use tracing::trace;

pub mod atomize;

impl SleighCompiler {
    /// Process a constructor. This will create the table in which the constructor is anchored, if it does not exist.
    /// Then, the bit pattern section is validated.
    pub(super) fn process_constructor(
        &mut self,
        constructor: Spanned<SASTConstructor>,
    ) -> SleighCompilerResult {
        let (constructor, span) = constructor.into_tuple();
        let table_name = constructor
            .table
            .value()
            .map(|sid| self.spec_builder.string(sid))
            .flatten()
            .unwrap_or("instruction");
        trace!("Defining constructor in table {:?}", table_name);
        let mut moir_constructor = MOIRConstructor {
            local_scope: Default::default(),
            instructions: vec![],
        };
        self.validate_constraint(constructor.bit_pattern_section.into_value().constraint)?;
        // TODO: we need to walk the constraint and find out the tokens that are used.
        //       we also need to make sure that there is only one token per section
        //       delimited by a semicolon.

        // The semantic section contains pcode which has to be converted into an atomic form.
        // Pcode can contain complex statements (e.g. x = (a+b) * c)) which have to be translated
        // into a form that consists only of atomic expressions (e.g. tmp = a + b; x = tmp * c;)
        let export = self.atomize_pcode(
            &mut moir_constructor.local_scope,
            &mut moir_constructor.instructions,
            constructor.semantic_section.into_value().0,
        )?;
        let table = match constructor.table.lift_option() {
            None => self.spec_builder.modify_root_table()?,
            Some(id) => self.spec_builder.modify_table(id)?,
        };
        if table.export.is_some() && export.is_none() {
            todo!("Generate error report for constructor missing an export statement")
        }
        table.export = export;
        Ok(())
    }

    fn validate_constraint(
        &self,
        constraint: Spanned<SASTBitPatternConstraint>,
    ) -> SleighCompilerResult {
        let (constraint, span) = constraint.into_tuple();
        match constraint {
            SASTBitPatternConstraint::Ident(_) => {}
            SASTBitPatternConstraint::BinaryOp(_, _, _) => {}
            SASTBitPatternConstraint::UnaryOp(_, _) => {}
            SASTBitPatternConstraint::AdvancedConstraint(_, _, _) => {}
            SASTBitPatternConstraint::Invalid => {}
        };
        Ok(())
    }
}
