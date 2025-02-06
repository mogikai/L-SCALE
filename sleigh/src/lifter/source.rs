use mogi_utilities::addresses::EVAddr;

// TODO: Move this to lifter trait file
pub trait TargetInstructionSource {
    type Error;

    /// Load instruction data beginning at the target emulated virtual address.
    /// If any part of the memory that is read is not executable, an error is returned.
    fn load(
        &self,
        addr: EVAddr,
        buffer: &mut [u8],
    ) -> Result<(), Self::Error>;
}

/// A simple helper struct to allow mapping instructions in a byte array.
/// The [EVAddr](mogi_utilities::addresses::EVAddr) refers to the offset
/// in the array after subtracting the base. I.e. the array is accessed at
/// `data[addr-base]`.
pub struct DirectlyMappedBytes<'a> {
    base: EVAddr,
    data: &'a [u8],
}

impl<'a> DirectlyMappedBytes<'a> {
    pub fn with_base(
        base: EVAddr,
        data: &'a [u8],
    ) -> Self {
        Self { base, data }
    }
}

impl<'a> From<&'a [u8]> for DirectlyMappedBytes<'a> {
    fn from(data: &'a [u8]) -> Self {
        DirectlyMappedBytes {
            base: EVAddr::null(),
            data,
        }
    }
}

impl<'a, const N: usize> From<&'a [u8; N]> for DirectlyMappedBytes<'a> {
    fn from(value: &'a [u8; N]) -> Self {
        DirectlyMappedBytes {
            base: EVAddr::null(),
            data: value.as_slice(),
        }
    }
}

impl<'a> TargetInstructionSource for DirectlyMappedBytes<'a> {
    type Error = ();

    fn load(
        &self,
        addr: EVAddr,
        buffer: &mut [u8],
    ) -> Result<(), Self::Error> {
        let offset = addr.checked_sub(&self.base).ok_or(())?;
        let data = self
            .data
            .split_at_checked(offset)
            .map(|(_, x)| x)
            .ok_or(())?;
        buffer.copy_from_slice(data);
        Ok(())
    }
}
