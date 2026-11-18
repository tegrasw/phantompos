//! Per-receipt byte ranges: what the app exports as a receipt's `.bin`.
//!
//! The backend cuts each connection stream at the offsets reported by
//! `SessionEvent::ReceiptCompleted { end, .. }`: a receipt owns the bytes
//! from the end of the previous receipt (or the stream start) to its own end.
//! Stage 6 acceptance: "o `.bin` exportado passa como fixture". Every slice
//! must replay on its own as a fixture would: it produces exactly one
//! receipt, closed the same way. The first receipt of a stream starts from
//! the power-on state, so its slice reproduces it exactly. Later receipts may
//! depend on state set by an earlier receipt (styles, code page, line
//! spacing: a cut is not a reset); that inherited state is not part of the
//! slice, so only their count and cut are compared (accepted caveat, owner
//! decision).

mod common;

use phantompos_core::PrinterStatus;

#[test]
fn receipt_slices_replay_as_fixtures() {
    let mut multi = 0;
    for bin in common::bin_fixtures() {
        let input = std::fs::read(&bin).expect("fixture is readable");
        let paper = common::paper_for(&bin);
        let status = common::status_for(&bin);
        let run = common::run_whole_in(&input, paper, status);
        let ends = &run.receipt_ends;
        assert_eq!(ends.len(), run.output.receipts.len(), "{}", bin.display());
        if ends.len() > 1 {
            multi += 1;
        }

        let mut start = 0_usize;
        for (index, (&end, receipt)) in ends.iter().zip(&run.output.receipts).enumerate() {
            let end = usize::try_from(end).expect("offset fits");
            assert!(
                start < end && end <= input.len(),
                "{}: receipt {index} range {start}..{end} of {}",
                bin.display(),
                input.len()
            );
            let slice = &input[start..end];
            let replay = common::run_whole_in(slice, paper, status);
            assert_eq!(
                replay.output.receipts.len(),
                1,
                "{}: slice {start}..{end} of receipt {index}",
                bin.display()
            );
            assert_eq!(replay.receipt_ends, vec![slice.len() as u64]);
            if index == 0 {
                assert_eq!(
                    &replay.output.receipts[0],
                    receipt,
                    "{}: first receipt",
                    bin.display()
                );
            } else {
                assert_eq!(replay.output.receipts[0].cut, receipt.cut);
            }
            start = end;
        }
        // Bytes after the last receipt belong to none: they produce no
        // receipt of their own.
        if start > 0 && start < input.len() && status == PrinterStatus::default() {
            let rest = common::run_whole_in(&input[start..], paper, status);
            assert!(
                rest.output.receipts.is_empty(),
                "{}: bytes after the last receipt printed something",
                bin.display()
            );
        }
    }
    assert!(multi >= 3, "only {multi} multi-receipt fixtures");
}
