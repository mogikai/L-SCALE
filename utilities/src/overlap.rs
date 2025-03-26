use std::borrow::Borrow;
use std::ops::Range;

/// Checks whether range `a` overlaps with range `b`
pub fn overlaps_generic<T, U, V>(
    a: &T,
    b: &U,
) -> bool
where
    Range<V>: for<'a> From<&'a T> + for<'a> From<&'a U>,
    V: PartialOrd,
{
    overlaps(Range::<V>::from(a), Range::<V>::from(b))
}

pub fn overlaps<V>(
    a: Range<V>,
    b: Range<V>,
) -> bool
where
    V: PartialOrd,
{
    // Ranges >dont< overlap if a ends before b, or b ends before a, i.e.:
    // b.end <= a.start || a.end <= b.offset
    // negate and reorder, and we have the following expression:
    a.start < b.end && b.start < a.end
}
