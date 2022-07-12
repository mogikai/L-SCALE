#[derive(Debug)]
pub enum IOErr {
    EINVAL,
    EISDIR,
    ENOENT,
}

pub trait Handle {
    /// returns: The stat vector of the handle this method is called on
    fn stat(&self) -> Vec<u8>;

    /// # Arguments
    /// * `count`: The amount of bytes to read from the current position of the descriptor
    ///
    /// returns: A reference to the data that is accessed by the read call
    fn read(&mut self, count: u64) -> Result<&[u8], IOErr>;

    /// # Arguments
    /// * `offset`: The offset to read from
    /// * `length`: The length of the data to be read
    ///
    /// returns: A reference to the data that is accessed by the pread call
    fn pread(&self, offset: u64, length: u64) -> Result<&[u8], IOErr>;

    fn write(&mut self, data: &[u8]) -> Result<usize, IOErr>;
}