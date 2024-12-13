#[macro_export]
macro_rules! jinst {
    (unimpl) => {
        $crate::spec::jir::jcode::JIRInstruction{
            output: None,
            input_a: None,
            input_b: None,
            operation: $crate::spec::jir::jcode::JIROperation::Unimpl,
        }
    };
    ($to:tt = $from:tt) => {
        $crate::spec::jir::jcode::JIRInstruction::new_unary_op($to, $from, $crate::spec::jir::jcode::JIROperation::Copy)
    };
    ($to:tt = *[$space:tt]$addr:tt) => {
        $crate::spec::jir::jcode::JIRInstruction::load($to, $addr, $space)
    };
    (*[$space:tt]$addr:tt = $value:tt) => {
        $crate::spec::jir::jcode::JIRInstruction::store($addr, $space, $value)
    };
    ($to:tt = popcount($from:tt)) => {
        $crate::spec::jir::jcode::JIRInstruction::new_unary_op(
            $to,
            $from,
            $crate::spec::jir::jcode::JIROperation::PopCount,
        )
    };
    ($to:tt = lzcount($from:tt)) => {
        $crate::spec::jir::jcode::JIRInstruction::new_unary_op(
            $to,
            $from,
            $crate::spec::jir::jcode::JIROperation::LZCount,
        )
    };
    ($to:tt = zext($from:tt)) => {
        $crate::spec::jir::jcode::JIRInstruction::new_unary_op(
            $to,
            $from,
            $crate::spec::jir::jcode::JIROperation::ZeroExtend,
        )
    };
    ($to:tt = sext($from:tt)) => {
        $crate::spec::jir::jcode::JIRInstruction::new_unary_op(
            $to,
            $from,
            $crate::spec::jir::jcode::JIROperation::SignExtend,
        )
    };
    ($to:tt = carry($a:tt, $b:tt)) => {
        $crate::spec::jir::jcode::JIRInstruction::new_binary_op(
            $to,
            $a,
            $b,
            $crate::spec::jir::jcode::JIROperation::Carry,
        )
    };
    ($to:tt = scarry($a:tt, $b:tt)) => {
        $crate::spec::jir::jcode::JIRInstruction::new_binary_op(
            $to,
            $a,
            $b,
            $crate::spec::jir::jcode::JIROperation::SignCarry,
        )
    };
    ($to:tt = sborrow($a:tt, $b:tt)) => {
        $crate::spec::jir::jcode::JIRInstruction::new_binary_op(
            $to,
            $a,
            $b,
            $crate::spec::jir::jcode::JIROperation::SignBorrow,
        )
    };
    ($to:tt = nan($from:tt)) => {
        $crate::spec::jir::jcode::JIRInstruction::new_unary_op($to, $from, $crate::spec::jir::jcode::JIROperation::Nan)
    };
    ($to:tt = abs($from:tt)) => {
        $crate::spec::jir::jcode::JIRInstruction::new_unary_op($to, $from, $crate::spec::jir::jcode::JIROperation::Abs)
    };
    ($to:tt = sqrt($from:tt)) => {
        $crate::spec::jir::jcode::JIRInstruction::new_unary_op($to, $from, $crate::spec::jir::jcode::JIROperation::Sqrt)
    };
    ($to:tt = int2float($from:tt)) => {
        $crate::spec::jir::jcode::JIRInstruction::new_unary_op(
            $to,
            $from,
            $crate::spec::jir::jcode::JIROperation::IntToFloat,
        )
    };
    ($to:tt = float2float($from:tt)) => {
        $crate::spec::jir::jcode::JIRInstruction::new_unary_op(
            $to,
            $from,
            $crate::spec::jir::jcode::JIROperation::FloatToFloat,
        )
    };
    ($to:tt = trunc($from:tt)) => {
        $crate::spec::jir::jcode::JIRInstruction::new_unary_op(
            $to,
            $from,
            $crate::spec::jir::jcode::JIROperation::Trunc,
        )
    };
    ($to:tt = ceil($from:tt)) => {
        $crate::spec::jir::jcode::JIRInstruction::new_unary_op($to, $from, $crate::spec::jir::jcode::JIROperation::Ceil)
    };
    ($to:tt = floor($from:tt)) => {
        $crate::spec::jir::jcode::JIRInstruction::new_unary_op(
            $to,
            $from,
            $crate::spec::jir::jcode::JIROperation::Floor,
        )
    };
    ($to:tt = round($from:tt)) => {
        $crate::spec::jir::jcode::JIRInstruction::new_unary_op(
            $to,
            $from,
            $crate::spec::jir::jcode::JIROperation::Round,
        )
    };
    ($to:tt = $a:tt + $b:tt) => {
        $crate::spec::jir::jcode::JIRInstruction::new_binary_op(
            $to,
            $a,
            $b,
            $crate::spec::jir::jcode::JIROperation::IntAdd,
        )
    };
    ($to:tt = $a:tt - $b:tt) => {
        $crate::spec::jir::jcode::JIRInstruction::new_binary_op(
            $to,
            $a,
            $b,
            $crate::spec::jir::jcode::JIROperation::IntSubtract,
        )
    };
    ($to:tt = $a:tt * $b:tt) => {
        $crate::spec::jir::jcode::JIRInstruction::new_binary_op(
            $to,
            $a,
            $b,
            $crate::spec::jir::jcode::JIROperation::IntMul,
        )
    };
    ($to:tt = $a:tt / $b:tt) => {
        $crate::spec::jir::jcode::JIRInstruction::new_binary_op(
            $to,
            $a,
            $b,
            $crate::spec::jir::jcode::JIROperation::IntDiv,
        )
    };
    ($to:tt = $a:tt s/ $b:tt) => {
        $crate::spec::jir::jcode::JIRInstruction::new_binary_op(
            $to,
            $a,
            $b,
            $crate::spec::jir::jcode::JIROperation::IntSDiv,
        )
    };
    ($to:tt = $a:tt % $b:tt) => {
        $crate::spec::jir::jcode::JIRInstruction::new_binary_op(
            $to,
            $a,
            $b,
            $crate::spec::jir::jcode::JIROperation::IntRemainder,
        )
    };
    ($to:tt = $a:tt s% $b:tt) => {
        $crate::spec::jir::jcode::JIRInstruction::new_binary_op(
            $to,
            $a,
            $b,
            $crate::spec::jir::jcode::JIROperation::IntSRemainder,
        )
    };
    ($to:tt = $a:tt f/ $b:tt) => {
        $crate::spec::jir::jcode::JIRInstruction::new_binary_op(
            $to,
            $a,
            $b,
            $crate::spec::jir::jcode::JIROperation::FloatDiv,
        )
    };
    ($to:tt = $a:tt f* $b:tt) => {
        $crate::spec::jir::jcode::JIRInstruction::new_binary_op(
            $to,
            $a,
            $b,
            $crate::spec::jir::jcode::JIROperation::FloatMul,
        )
    };
    ($to:tt = $a:tt f+ $b:tt) => {
        $crate::spec::jir::jcode::JIRInstruction::new_binary_op(
            $to,
            $a,
            $b,
            $crate::spec::jir::jcode::JIROperation::FloatAdd,
        )
    };
    ($to:tt = $a:tt f- $b:tt) => {
        $crate::spec::jir::jcode::JIRInstruction::new_binary_op(
            $to,
            $a,
            $b,
            $crate::spec::jir::jcode::JIROperation::FloatSubtract,
        )
    };
    ($to:tt = $a:tt || $b:tt) => {
        $crate::spec::jir::jcode::JIRInstruction::new_binary_op(
            $to,
            $a,
            $b,
            $crate::spec::jir::jcode::JIROperation::BoolOr,
        )
    };
    ($to:tt = $a:tt && $b:tt) => {
        $crate::spec::jir::jcode::JIRInstruction::new_binary_op(
            $to,
            $a,
            $b,
            $crate::spec::jir::jcode::JIROperation::BoolAnd,
        )
    };
    ($to:tt = $a:tt ^^ $b:tt) => {
        $crate::spec::jir::jcode::JIRInstruction::new_binary_op(
            $to,
            $a,
            $b,
            $crate::spec::jir::jcode::JIROperation::BoolXor,
        )
    };
    ($to:tt = $a:tt << $b:tt) => {
        $crate::spec::jir::jcode::JIRInstruction::new_binary_op(
            $to,
            $a,
            $b,
            $crate::spec::jir::jcode::JIROperation::IntLeft,
        )
    };
    ($to:tt = $a:tt >> $b:tt) => {
        $crate::spec::jir::jcode::JIRInstruction::new_binary_op(
            $to,
            $a,
            $b,
            $crate::spec::jir::jcode::JIROperation::IntRight,
        )
    };
    ($to:tt = $a:tt s>> $b:tt) => {
        $crate::spec::jir::jcode::JIRInstruction::new_binary_op(
            $to,
            $a,
            $b,
            $crate::spec::jir::jcode::JIROperation::IntSRight,
        )
    };
    ($to:tt = $a:tt s< $b:tt) => {
        $crate::spec::jir::jcode::JIRInstruction::new_binary_op(
            $to,
            $a,
            $b,
            $crate::spec::jir::jcode::JIROperation::IntSLess,
        )
    };
    ($to:tt = $a:tt s> $b:tt) => {
        $crate::spec::jir::jcode::JIRInstruction::new_binary_op(
            $to,
            $a,
            $b,
            $crate::spec::jir::jcode::JIROperation::IntSGreater,
        )
    };
    ($to:tt = $a:tt s<= $b:tt) => {
        $crate::spec::jir::jcode::JIRInstruction::new_binary_op(
            $to,
            $a,
            $b,
            $crate::spec::jir::jcode::JIROperation::IntSLessEqual,
        )
    };
    ($to:tt = $a:tt s>= $b:tt) => {
        $crate::spec::jir::jcode::JIRInstruction::new_binary_op(
            $to,
            $a,
            $b,
            $crate::spec::jir::jcode::JIROperation::IntSGreaterEqual,
        )
    };
    ($to:tt = $a:tt < $b:tt) => {
        $crate::spec::jir::jcode::JIRInstruction::new_binary_op(
            $to,
            $a,
            $b,
            $crate::spec::jir::jcode::JIROperation::IntLess,
        )
    };
    ($to:tt = $a:tt > $b:tt) => {
        $crate::spec::jir::jcode::JIRInstruction::new_binary_op(
            $to,
            $a,
            $b,
            $crate::spec::jir::jcode::JIROperation::IntGreater,
        )
    };
    ($to:tt = $a:tt <= $b:tt) => {
        $crate::spec::jir::jcode::JIRInstruction::new_binary_op(
            $to,
            $a,
            $b,
            $crate::spec::jir::jcode::JIROperation::IntLessEqual,
        )
    };
    ($to:tt = $a:tt >= $b:tt) => {
        $crate::spec::jir::jcode::JIRInstruction::new_binary_op(
            $to,
            $a,
            $b,
            $crate::spec::jir::jcode::JIROperation::IntGreaterEqual,
        )
    };
    ($to:tt = $a:tt f< $b:tt) => {
        $crate::spec::jir::jcode::JIRInstruction::new_binary_op(
            $to,
            $a,
            $b,
            $crate::spec::jir::jcode::JIROperation::FloatLess,
        )
    };
    ($to:tt = $a:tt f> $b:tt) => {
        $crate::spec::jir::jcode::JIRInstruction::new_binary_op(
            $to,
            $a,
            $b,
            $crate::spec::jir::jcode::JIROperation::FloatGreater,
        )
    };
    ($to:tt = $a:tt f<= $b:tt) => {
        $crate::spec::jir::jcode::JIRInstruction::new_binary_op(
            $to,
            $a,
            $b,
            $crate::spec::jir::jcode::JIROperation::FloatLessEqual,
        )
    };
    ($to:tt = $a:tt f>= $b:tt) => {
        $crate::spec::jir::jcode::JIRInstruction::new_binary_op(
            $to,
            $a,
            $b,
            $crate::spec::jir::jcode::JIROperation::FloatGreaterEqual,
        )
    };
    ($to:tt = $a:tt f== $b:tt) => {
        $crate::spec::jir::jcode::JIRInstruction::new_binary_op(
            $to,
            $a,
            $b,
            $crate::spec::jir::jcode::JIROperation::FloatEquals,
        )
    };
    ($to:tt = $a:tt f!= $b:tt) => {
        $crate::spec::jir::jcode::JIRInstruction::new_binary_op(
            $to,
            $a,
            $b,
            $crate::spec::jir::jcode::JIROperation::FloatNotEquals,
        )
    };
    ($to:tt = $a:tt | $b:tt) => {
        $crate::spec::jir::jcode::JIRInstruction::new_binary_op(
            $to,
            $a,
            $b,
            $crate::spec::jir::jcode::JIROperation::IntOr,
        )
    };
    ($to:tt = $a:tt & $b:tt) => {
        $crate::spec::jir::jcode::JIRInstruction::new_binary_op(
            $to,
            $a,
            $b,
            $crate::spec::jir::jcode::JIROperation::IntAnd,
        )
    };
    ($to:tt = $a:tt ^ $b:tt) => {
        $crate::spec::jir::jcode::JIRInstruction::new_binary_op(
            $to,
            $a,
            $b,
            $crate::spec::jir::jcode::JIROperation::IntXor,
        )
    };
    ($to:tt = $a:tt == $b:tt) => {
        $crate::spec::jir::jcode::JIRInstruction::new_binary_op(
            $to,
            $a,
            $b,
            $crate::spec::jir::jcode::JIROperation::IntEquals,
        )
    };
    ($to:tt = $a:tt != $b:tt) => {
        $crate::spec::jir::jcode::JIRInstruction::new_binary_op(
            $to,
            $a,
            $b,
            $crate::spec::jir::jcode::JIROperation::IntNotEquals,
        )
    };
    ($to:tt = !$from:tt) => {
        $crate::spec::jir::jcode::JIRInstruction::new_unary_op(
            $to,
            $from,
            $crate::spec::jir::jcode::JIROperation::BoolNegate,
        )
    };
    ($to:tt = ~$from:tt) => {
        $crate::spec::jir::jcode::JIRInstruction::new_unary_op(
            $to,
            $from,
            $crate::spec::jir::jcode::JIROperation::BitNegate,
        )
    };
    ($to:tt = -$from:tt) => {
        $crate::spec::jir::jcode::JIRInstruction::new_unary_op(
            $to,
            $from,
            $crate::spec::jir::jcode::JIROperation::IntNegate,
        )
    };
    ($to:tt = fneg $from:tt) => {
        $crate::spec::jir::jcode::JIRInstruction::new_unary_op(
            $to,
            $from,
            $crate::spec::jir::jcode::JIROperation::FloatNegate,
        )
    };
    (goto $addr:block) => {
        $crate::spec::jir::jcode::JIRInstruction::new_branch(
            $addr,
            $crate::spec::jir::jcode::JIROperation::GotoDirect,
        )
    };
    (goto $addr:ident) => {
        jinst!(goto {$addr})
    };
    (goto [$addr:block]) => {
        $crate::spec::jir::jcode::JIRInstruction::new_branch(
            $addr,
            $crate::spec::jir::jcode::JIROperation::GotoIndirect,
        )
    };
    (goto [$addr:ident]) => {
        jinst!(goto [{$addr}])
    };
    (goto <$label:block>) => {
        $crate::spec::jir::jcode::JIRInstruction::new_jcode_branch(
            Option::<JIRValue>::None,
            $crate::spec::jir::jcode::JIROperation::JCodeGoto($label),
        )
    };
    (goto <$label:ident>) => {
        jinst!(goto <{$label}>)
    };
    (call $addr:block) => {
        $crate::spec::jir::jcode::JIRInstruction::new_branch(
            $addr,
            $crate::spec::jir::jcode::JIROperation::CallDirect,
        )
    };
    (call $addr:ident) => {
        jinst!(call {$addr})
    };
    (call [$addr:block]) => {
        $crate::spec::jir::jcode::JIRInstruction::new_branch(
            $addr,
            $crate::spec::jir::jcode::JIROperation::CallIndirect,
        )
    };
    (call [$addr:ident]) => {
        jinst!(call [{$addr}])
    };
    (return $addr:block) => {
        $crate::spec::jir::jcode::JIRInstruction::new_branch(
            $addr,
            $crate::spec::jir::jcode::JIROperation::ReturnDirect,
        )
    };
    (return $addr:ident) => {
        jinst!(return {$addr})
    };
    (return [$addr:block]) => {
        $crate::spec::jir::jcode::JIRInstruction::new_branch(
            $addr,
            $crate::spec::jir::jcode::JIROperation::ReturnIndirect,
        )
    };
    (return [$addr:ident]) => {
        jinst!(return {$addr})
    };
    (if $cond:tt goto $addr:block) => {
        $crate::spec::jir::jcode::JIRInstruction::new_conditional_branch(
            $addr,
            $cond,
            $crate::spec::jir::jcode::JIROperation::GotoDirectIf,
        )
    };
    (if $cond:tt goto $addr:ident) => {
        jinst!(if $cond goto {$addr})
    };
    (if $cond:tt goto [$addr:block]) => {
        $crate::spec::jir::jcode::JIRInstruction::new_conditional_branch(
            $addr,
            $cond,
            $crate::spec::jir::jcode::JIROperation::GotoIndirectIf,
        )
    };
    (if $cond:tt goto [$addr:ident]) => {
        jinst!(if $cond goto [{$addr}])
    };
    (if $cond:tt goto <$label:block>) => {
        $crate::spec::jir::jcode::JIRInstruction::new_jcode_branch(
            Some($cond),
            $crate::spec::jir::jcode::JIROperation::JCodeGotoIf($label),
        )
    };
    (if $cond:tt goto <$label:ident>) => {
        jinst!(if $cond goto {$label})
    };
    ($out:tt = (*$op_id:tt)($($arg:tt),*)) => {
        $crate::spec::jir::jcode::JIRInstruction::new_custom_call(Some($out), $op_id, vec![$($arg),*])
    };
    ((*$op_id:tt)($($arg:tt),*)) => {
        $crate::spec::jir::jcode::JIRInstruction::new_custom_call(Option::<JIRSlice>::None, $op_id, vec![$($arg),*])
    };
    ($out:tt = (*$op_id:tt)(*$args:tt)) => {
        $crate::spec::jir::jcode::JIRInstruction::new_custom_call(Some($out), $op_id, $args)
    };
    ((*$op_id:tt)(*$args:tt)) => {
        $crate::spec::jir::jcode::JIRInstruction::new_custom_call(Option::<JIRSlice>::None, $op_id, $args)
    };
}
use crate::spec::jir::values::JIRValue;
pub use jinst;
