//! Record-buffer capacity policy; mirrors the private native layout.
use nil_hir::{Diagnostic, MAX_DYNAMIC_BYTES, Span};
pub(crate) fn capacity(
    cap: usize,
    a: usize,
    b: usize,
    rhs: usize,
    width: usize,
    span: Option<Span>,
) -> Result<usize, Diagnostic> {
    let fail = || crate::application::fault("E013", span, "dynamic allocation limit exceeded");
    let maximum = (MAX_DYNAMIC_BYTES - 40) / width - 1;
    let needed = a
        .checked_add(b)
        .filter(|n| *n <= maximum)
        .ok_or_else(fail)?;
    if needed <= cap {
        return Ok(cap);
    }
    let occupied = 3 * (40 + width) + rhs * width;
    let steady = MAX_DYNAMIC_BYTES.saturating_sub(occupied) / (2 * width);
    let grown = if cap > steady / 2 { steady } else { cap * 2 };
    Ok(needed.max(grown))
}
