use thiserror::Error;

use crate::cpu::Cpu;

#[derive(Debug, Error)]
pub enum CpuError {
    #[error("The requested register does not exist")]
    UnknownRegister,
    #[error("An unknown error occurred")]
    Unknown,
}

type CpuResult<T> = Result<T, CpuError>;

impl Cpu {
    pub fn read_reg(
        &self,
        reg: &str,
    ) -> CpuResult<u64> {
        let reg = self
            .core
            .arch
            .sleigh
            .get_reg(reg)
            .ok_or(CpuError::UnknownRegister)?
            .var;
        Ok(self.core.read_reg(reg))
    }

    pub fn write_reg(
        &mut self,
        reg: &str,
        val: u64,
    ) -> CpuResult<()> {
        self.core.write_reg(reg, val)
    }

    pub fn read_pc(&self) -> CpuResult<u64> {
        Ok(self.core.read_pc())
    }

    pub fn write_pc(
        &mut self,
        val: u64,
    ) -> CpuResult<()> {
        self.core.write_pc(val);
        Ok(())
    }
}
