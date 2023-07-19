#[derive(Debug)]
pub enum FileDescriptorMapError {
    InvalidFD,
    NoFreeFD,
    FDExists,
}
