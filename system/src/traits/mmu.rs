use mogi_utilities::addresses::EPAddr;
use mogi_utilities::error::memory::MemoryError;

// TODO: Extract memory error from utilities into system crate?

pub type MmuResult<T> = Result<T, MemoryError>;

pub trait MMU {
    fn map_physical(
        &mut self,
        address: EPAddr,
    ) -> MmuResult<()>;

    /// Reads the specified amount of bytes from the emulated physical memory, starting at `address`.
    ///
    /// # Arguments
    ///
    /// * `address`: The emulated physical address to start reading from.
    /// * `len`: The amount of bytes to read
    ///
    /// returns: A vector of bytes that were read from memory,
    fn read_physical(
        &self,
        address: EPAddr,
        len: usize,
    ) -> MmuResult<Vec<u8>>;

    /// Does the same thing as [Self::read_physical], but starts at
    /// the end of the memory area, and reads downwards.
    fn read_physical_down(
        &self,
        address: EPAddr,
        len: usize,
    ) -> MmuResult<Vec<u8>> {
        self.read_physical(address - len, len)
    }

    /// Writes the provided data to the emulated physical memory
    /// starting at the provided emulated physical address.
    ///
    /// # Arguments
    ///
    /// * `address`: The emulated physical address to write the data to
    /// * `data`: The data to be written.
    ///
    /// returns: The amount of data written. If a failure occurred, a [MemoryError] is returned.
    fn write_physical(
        &mut self,
        address: EPAddr,
        data: &[u8],
    ) -> MmuResult<usize>;

    /// Does the same thing as [Self::write_physical], but starts writing at the end
    /// of the memory and writes downwards.
    fn write_physical_down(
        &mut self,
        address: EPAddr,
        data: &[u8],
    ) -> MmuResult<usize> {
        self.write_physical(address - data.len(), data)
    }
}
