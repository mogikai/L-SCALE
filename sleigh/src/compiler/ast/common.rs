#[derive(Debug)]
pub struct SASTIdent<'src>(pub &'src str);
#[derive(Debug)]
pub struct SASTString<'src>(pub &'src str);
