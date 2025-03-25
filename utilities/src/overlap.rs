use std::ops::Range;

/// Checks whether range `a` overlaps with range `b`
pub fn overlaps<T, U, V>(
    a: &T,
    b: &U,
) -> bool
where
    Range<V>: for<'a> From<&'a T> + for<'a> From<&'a U>,
    V: PartialOrd,
{
    // Ranges >dont< overlap if a ends before b, or b ends before a, i.e.:
    // b.end <= a.start || a.end <= b.offset
    // negate and reorder, and we have the following expression:
    let a = Range::<V>::from(a);
    let b = Range::<V>::from(b);
    a.start < b.end && b.start < a.end
}
