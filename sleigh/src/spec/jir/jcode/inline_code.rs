#[macro_export]
macro_rules! jinst {
    ($to:tt = $from:tt) => {
        $crate::spec::jir::jcode::JIRInstruction::copy($to, $from)
    };
    ($to:tt = *[$space:tt]$addr:tt) => {
        $crate::spec::jir::jcode::JIRInstruction::load($to, $addr, $space)
    };
    (*[$space:tt]$addr:tt = $value:tt) => {
        $crate::spec::jir::jcode::JIRInstruction::store($addr, $space, $value)
    };
    ($to:tt = popcount($from:tt)) => {
        $crate::spec::jir::jcode::JIRInstruction::pop_count($to, $from)
    };
    ($to:tt = lzcount($from:tt)) => {
        $crate::spec::jir::jcode::JIRInstruction::lzcount($to, $from)
    };
    ($to:tt = zext($from:tt)) => {
        $crate::spec::jir::jcode::JIRInstruction::zero_extend($to, $from)
    };
    ($to:tt = sext($from:tt)) => {
        $crate::spec::jir::jcode::JIRInstruction::sign_extend($to, $from)
    };
    ($to:tt = carry($a:tt, $b:tt)) => {
        $crate::spec::jir::jcode::JIRInstruction::carry($to, $a, $b)
    };
    ($to:tt = scarry($a:tt, $b:tt)) => {
        $crate::spec::jir::jcode::JIRInstruction::sign_carry($to, $a, $b)
    };
    ($to:tt = sborrow($a:tt, $b:tt)) => {
        $crate::spec::jir::jcode::JIRInstruction::sign_borrow($to, $a, $b)
    };
    ($to:tt = nan($from:tt)) => {
        $crate::spec::jir::jcode::JIRInstruction::nan($to, $from)
    };
    ($to:tt = abs($from:tt)) => {
        $crate::spec::jir::jcode::JIRInstruction::nan($to, $from)
    };
    ($to:tt = sqrt($from:tt)) => {
        $crate::spec::jir::jcode::JIRInstruction::sqrt($to, $from)
    };
    ($to:tt = int2float($from:tt)) => {
        $crate::spec::jir::jcode::JIRInstruction::int_to_float($to, $from)
    };
    ($to:tt = float2float($from:tt)) => {
        $crate::spec::jir::jcode::JIRInstruction::float_to_float($to, $from)
    };
    ($to:tt = trunc($from:tt)) => {
        $crate::spec::jir::jcode::JIRInstruction::trunc($to, $from)
    };
    ($to:tt = ceil($from:tt)) => {
        $crate::spec::jir::jcode::JIRInstruction::ceil($to, $from)
    };
    ($to:tt = floor($from:tt)) => {
        $crate::spec::jir::jcode::JIRInstruction::floor($to, $from)
    };
    ($to:tt = round($from:tt)) => {
        $crate::spec::jir::jcode::JIRInstruction::round($to, $from)
    };
    ($to:tt = $a:tt + $b:tt) => {
        $crate::spec::jir::jcode::JIRInstruction::int_add($to, $a, $b)
    };
    ($to:tt = $a:tt - $b:tt) => {
        $crate::spec::jir::jcode::JIRInstruction::int_subtract($to, $a, $b)
    };
    ($to:tt = $a:tt * $b:tt) => {
        $crate::spec::jir::jcode::JIRInstruction::int_mul($to, $a, $b)
    };
    ($to:tt = $a:tt / $b:tt) => {
        $crate::spec::jir::jcode::JIRInstruction::int_div($to, $a, $b)
    };
    ($to:tt = $a:tt s/ $b:tt) => {
        $crate::spec::jir::jcode::JIRInstruction::int_sdiv($to, $a, $b)
    };
    ($to:tt = $a:tt % $b:tt) => {
        $crate::spec::jir::jcode::JIRInstruction::int_remainder($to, $a, $b)
    };
    ($to:tt = $a:tt s% $b:tt) => {
        $crate::spec::jir::jcode::JIRInstruction::int_sremainder($to, $a, $b)
    };
    ($to:tt = $a:tt f/ $b:tt) => {
        $crate::spec::jir::jcode::JIRInstruction::float_div($to, $a, $b)
    };
    ($to:tt = $a:tt f* $b:tt) => {
        $crate::spec::jir::jcode::JIRInstruction::float_mul($to, $a, $b)
    };
    ($to:tt = $a:tt f+ $b:tt) => {
        $crate::spec::jir::jcode::JIRInstruction::float_add($to, $a, $b)
    };
    ($to:tt = $a:tt f- $b:tt) => {
        $crate::spec::jir::jcode::JIRInstruction::float_subtract($to, $a, $b)
    };
    ($to:tt = $a:tt || $b:tt) => {
        $crate::spec::jir::jcode::JIRInstruction::bool_or($to, $a, $b)
    };
    ($to:tt = $a:tt && $b:tt) => {
        $crate::spec::jir::jcode::JIRInstruction::bool_and($to, $a, $b)
    };
    ($to:tt = $a:tt ^^ $b:tt) => {
        $crate::spec::jir::jcode::JIRInstruction::bool_xor($to, $a, $b)
    };
    ($to:tt = $a:tt << $b:tt) => {
        $crate::spec::jir::jcode::JIRInstruction::int_left($to, $a, $b)
    };
    ($to:tt = $a:tt >> $b:tt) => {
        $crate::spec::jir::jcode::JIRInstruction::int_right($to, $a, $b)
    };
    ($to:tt = $a:tt s>> $b:tt) => {
        $crate::spec::jir::jcode::JIRInstruction::int_sright($to, $a, $b)
    };
    ($to:tt = $a:tt s< $b:tt) => {
        $crate::spec::jir::jcode::JIRInstruction::int_sless($to, $a, $b)
    };
    ($to:tt = $a:tt s> $b:tt) => {
        $crate::spec::jir::jcode::JIRInstruction::int_sgreater($to, $a, $b)
    };
    ($to:tt = $a:tt s<= $b:tt) => {
        $crate::spec::jir::jcode::JIRInstruction::int_sless_equal($to, $a, $b)
    };
    ($to:tt = $a:tt s>= $b:tt) => {
        $crate::spec::jir::jcode::JIRInstruction::int_sgreater_equal($to, $a, $b)
    };
    ($to:tt = $a:tt < $b:tt) => {
        $crate::spec::jir::jcode::JIRInstruction::int_less($to, $a, $b)
    };
    ($to:tt = $a:tt > $b:tt) => {
        $crate::spec::jir::jcode::JIRInstruction::int_greater($to, $a, $b)
    };
    ($to:tt = $a:tt <= $b:tt) => {
        $crate::spec::jir::jcode::JIRInstruction::int_less_equal($to, $a, $b)
    };
    ($to:tt = $a:tt >= $b:tt) => {
        $crate::spec::jir::jcode::JIRInstruction::int_greater_equal($to, $a, $b)
    };
    ($to:tt = $a:tt f< $b:tt) => {
        $crate::spec::jir::jcode::JIRInstruction::float_less($to, $a, $b)
    };
    ($to:tt = $a:tt f> $b:tt) => {
        $crate::spec::jir::jcode::JIRInstruction::float_greater($to, $a, $b)
    };
    ($to:tt = $a:tt f<= $b:tt) => {
        $crate::spec::jir::jcode::JIRInstruction::float_less_equal($to, $a, $b)
    };
    ($to:tt = $a:tt f>= $b:tt) => {
        $crate::spec::jir::jcode::JIRInstruction::float_greater_equal($to, $a, $b)
    };
    ($to:tt = $a:tt f== $b:tt) => {
        $crate::spec::jir::jcode::JIRInstruction::float_equals($to, $a, $b)
    };
    ($to:tt = $a:tt f!= $b:tt) => {
        $crate::spec::jir::jcode::JIRInstruction::float_not_equals($to, $a, $b)
    };
    ($to:tt = $a:tt | $b:tt) => {
        $crate::spec::jir::jcode::JIRInstruction::int_or($to, $a, $b)
    };
    ($to:tt = $a:tt & $b:tt) => {
        $crate::spec::jir::jcode::JIRInstruction::int_and($to, $a, $b)
    };
    ($to:tt = $a:tt ^ $b:tt) => {
        $crate::spec::jir::jcode::JIRInstruction::int_xor($to, $a, $b)
    };
    ($to:tt = $a:tt == $b:tt) => {
        $crate::spec::jir::jcode::JIRInstruction::int_equals($to, $a, $b)
    };
    ($to:tt = $a:tt != $b:tt) => {
        $crate::spec::jir::jcode::JIRInstruction::int_not_equals($to, $a, $b)
    };
    ($to:tt = !$from:tt) => {
        $crate::spec::jir::jcode::JIRInstruction::bool_negate($to, $from)
    };
    ($to:tt = ~$from:tt) => {
        $crate::spec::jir::jcode::JIRInstruction::bit_negate($to, $from)
    };
    ($to:tt = -$from:tt) => {
        $crate::spec::jir::jcode::JIRInstruction::int_negate($to, $from)
    };
    ($to:tt = fneg $from:tt) => {
        $crate::spec::jir::jcode::JIRInstruction::float_negate($to, $from)
    };
    (goto $addr:block) => {
        $crate::spec::jir::jcode::JIRInstruction::goto_direct($addr)
    };
    (goto $addr:ident) => {
        jinst!(goto {$addr})
    };
    (goto [$addr:block]) => {
        $crate::spec::jir::jcode::JIRInstruction::goto_indirect($addr)
    };
    (goto [$addr:ident]) => {
        jinst!(goto [{$addr}])
    };
    (goto <$label:block>) => {
        $crate::spec::jir::jcode::JIRInstruction::jcode_goto($label)
    };
    (goto <$label:ident>) => {
        jinst!(goto <{$label}>)
    };
    (call $addr:block, $retaddr:block) => {
        $crate::spec::jir::jcode::JIRInstruction::call_direct($addr, $retaddr)
    };
    (call $addr:ident, $retaddr:block) => {
        jinst!(call {$addr}, $retaddr)
    };
    (call [$addr:block], $retaddr:block) => {
        $crate::spec::jir::jcode::JIRInstruction::call_indirect($addr, $retaddr)
    };
    (call [$addr:ident], $retaddr:block) => {
        jinst!(call [{$addr}], $retaddr)
    };
    (return $addr:block) => {
        $crate::spec::jir::jcode::JIRInstruction::return_direct($addr)
    };
    (return $addr:ident) => {
        jinst!(return {$addr})
    };
    (return [$addr:block]) => {
        $crate::spec::jir::jcode::JIRInstruction::return_indirect($addr)
    };
    (return [$addr:ident]) => {
        jinst!(return {$addr})
    };
    (if $cond:tt goto $addr:block) => {
        $crate::spec::jir::jcode::JIRInstruction::goto_direct_if($addr,  $cond)
    };
    (if $cond:tt goto $addr:ident) => {
        jinst!(if $cond goto {$addr})
    };
    (if $cond:tt goto [$addr:block]) => {
        $crate::spec::jir::jcode::JIRInstruction::goto_indirect_if($addr, $cond)
    };
    (if $cond:tt goto [$addr:ident]) => {
        jinst!(if $cond goto [{$addr}])
    };
    (if $cond:tt goto <$label:block>) => {
        $crate::spec::jir::jcode::JIRInstruction::jcode_goto_if($cond, $label)
    };
    (if $cond:tt goto <$label:ident>) => {
        jinst!(if $cond goto {$label})
    };
    ($out:tt = (*$op_id:tt)($($arg:tt),*)) => {
        $crate::spec::jir::jcode::JIRInstruction::new_custom_call(Some($out), $op_id, vec![$($crate::spec::jir::values::JIRValue::from($arg)),*])
    };
    ((*$op_id:tt)($($arg:tt),*)) => {
        $crate::spec::jir::jcode::JIRInstruction::new_custom_call(Option::<JIRSlice>::None, $op_id, vec![$($crate::spec::jir::values::JIRValue::from($arg)),*])
    };
    ($out:tt = (*$op_id:tt)(*$args:tt)) => {
        $crate::spec::jir::jcode::JIRInstruction::new_custom_call(Some($out), $op_id, $args)
    };
    ((*$op_id:tt)(*$args:tt)) => {
        $crate::spec::jir::jcode::JIRInstruction::new_custom_call(Option::<JIRSlice>::None, $op_id, $args)
    };
}
pub use jinst;
