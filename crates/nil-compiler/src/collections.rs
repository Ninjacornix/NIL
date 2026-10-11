//! Geometric record-buffer allocation, not semantic capacity accounting.
use nil_hir::{Diagnostic, Span};
pub(crate) fn capacity(
    cap: usize,
    a: usize,
    b: usize,
    width: usize,
    limit: usize,
    span: Option<Span>,
) -> Result<usize, Diagnostic> {
    let fail = || crate::application::fault("E013", span, "dynamic allocation guard exceeded");
    let maximum = limit
        .saturating_sub(40)
        .checked_div(width)
        .and_then(|n| n.checked_sub(1))
        .ok_or_else(fail)?;
    let needed = a
        .checked_add(b)
        .filter(|n| *n <= maximum)
        .ok_or_else(fail)?;
    Ok(if needed <= cap {
        cap
    } else {
        needed.max(cap.saturating_mul(2).min(maximum))
    })
}
