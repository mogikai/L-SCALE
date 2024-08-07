use crate::new_symbol_id_type;
use crate::spec::moir::varnode::MOIRVarNodeId;

#[derive(Debug, Default)]
pub struct MOIRTable {
    // TODO: how to actually implement this?
    // Ideally, each constructor inserts its bit pattern into a jump table
    // The first pattern in the op points to the second pattern and so on until
    // we reach the disassembly actions matchers. This can be a list, if multiple constructors
    // match. We then have to match each of these sections, and use the one with the most
    // constraints to finally lift the instruction.
    bit_matching_table: Vec<Vec<()>>,
    constructors: Vec<()>,
    /// TODO: improve documentation
    /// 7.7.2.3. Exports
    ///
    /// The semantic section doesn’t just specify how to generate p-code for a constructor.
    /// Except for those constructors in the root table, this section also associates a
    /// semantic meaning to the table symbol the constructor is part of, allowing the
    /// table to be used as an operand in other tables. The mechanism for making this
    /// association is the export statement. This must be the last statement in the section
    /// and consists of the export keyword followed by the specific symbol to be associated
    /// with the constructor. In general, the constructor will have a sequence of assignment
    /// statements building a final value, and then the varnode containing the value will
    /// be exported. However, anything can be exported.
    ///
    /// `mode: reg++ is addrmode=0x2 & reg { tmp=reg; reg=reg+1; export tmp; }`
    ///
    /// This is an example of a post-increment addressing mode that would be used to build more
    /// complicated instructions. The constructor increments a register reg but stores a copy of
    /// its original value in tmp. The varnode tmp is then exported, associating it with the
    /// table symbol mode. When this constructor is matched, as part of a more complicated instruction,
    /// the symbol mode will represent the original semantic value of reg but with the standard
    /// post-increment side effect.
    ///
    /// The table symbol associated with the constructor becomes a reference to the varnode being exported,
    /// not a copy of the value. If the table symbol is written to, as the left-hand side of an assignment
    /// statement, in some other constructor, the exported varnode is affected. A constant can be exported
    /// if its size as a varnode is given explicitly with the ‘:’ operator.
    ///
    /// It is not legal to put a full expression in an export statement, any expression must appear in an
    /// earlier statement. However, a single ‘&’ operator is allowed as part of the statement, and it behaves
    /// as it would in a normal expression (see Section 7.7.1.6, “Address-of Operator”). It causes the
    /// address of the varnode being modified to be exported as an integer constant.
    export: Option<MOIRVarNodeId>,
}

new_symbol_id_type! { pub struct MOIRTableId; }
