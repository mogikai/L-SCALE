use std::ops::Range;

/// Checks whether range `a` overlaps with range `b`
pub fn overlaps<T: Into<Range<V>>, U: Into<Range<V>>, V: PartialOrd>(
    a: T,
    b: U,
) -> bool {
    // Ranges >dont< overlap if a ends before b, or b ends before a, i.e.:
    // b.end <= a.start || a.end <= b.offset
    // negate and reorder, and we have the following expression:
    let a = a.into();
    let b = b.into();
    a.start < b.end && b.start < a.end
}
