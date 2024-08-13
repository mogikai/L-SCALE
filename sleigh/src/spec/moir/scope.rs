use crate::spec::moir::field::MOIRFieldId;
use crate::spec::moir::table::MOIRTableId;

/// A scope for locals used in constructors and macros
#[derive(Debug)]
pub struct MOIRScope {
    /// The local fields defined in either the constraint expression or as part of disassembly
    /// actions.
    pub fields: Vec<MOIRFieldId>,

    /// The subtables referenced by this constructor.
    pub subtables: Vec<MOIRTableId>,

    /// Keeps track of the sizes of temporaries assigned as part of the semantic section.
    temporaries: Vec<()>,
    locals: Vec<MOIRLocal>,
}

#[derive(Debug)]
pub struct MOIRLocal {}
