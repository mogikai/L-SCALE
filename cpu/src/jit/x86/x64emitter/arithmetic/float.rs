#[cfg(test)]
mod test;

mod binops;
mod itof;
mod round;
mod trunc;
mod ftof;

        // match (in_size, out_size) {
        //     (4, 4) => {
        //         // single-precision float to DWORD integer
        //         assemble!(self.block.assembler();
        //             ; cvtss2si Rd(output_reg.hw_enc()), Rx(input_reg.hw_enc())
        //         );
        //     }
        //     (4, 8) => {
        //         // single-precision float to QWORD integer
        //         assemble!(self.block.assembler();
        //             ; cvtss2si Rq(output_reg.hw_enc()), Rx(input_reg.hw_enc())
        //         );
        //     }
        //     (8, 4) => {
        //         // double-precision float to DWORD integer
        //         assemble!(self.block.assembler();
        //             ; cvtsd2si Rd(output_reg.hw_enc()), Rx(input_reg.hw_enc())
        //         );
        //     }
        //     (8, 8) => {
        //         // double-precision float to QWORD integer
        //         assemble!(self.block.assembler();
        //             ; cvtsd2si Rq(output_reg.hw_enc()), Rx(input_reg.hw_enc())
        //         );
        //     }
        //     size => todo!("Unsupported size {:?}", size),
        // }

        // Ok(())
    // }
