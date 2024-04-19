pub trait TLB {
    //     /// Instruct the tlb to generate a code block that performs inline access to the tlb
    //     /// If the tlb lookup fails, the runtime lookup function will be called to handle the tlb miss.
    //     /// The addr parameter is a jit variable that will contain the EVAddr to translate.
    //     /// The return value is a jit variable that will contain the translated address, i.e.
    //     /// the pointer to host virtual memory (HVAddr).
    //     fn generate_inline_access_instructions<TranslatorT, EVAddrValueT, BlockT, HVAddrValueT>(
    //         &self,
    //         trans: &mut TranslatorT,
    //         addr: EVAddrValueT,
    //         success_block: BlockT,
    //         failure_block: BlockT,
    //     ) -> HVAddrValueT;
    fn region(&self);
    fn region_mut(&self);
}
