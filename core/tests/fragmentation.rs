//! Fragmented input must give exactly the same result as the whole input
//! (commands, receipts, drawer kicks, replies and discarded bytes), for the
//! fixture's fixed printer state.

mod common;

use common::Rng;

#[test]
fn fixtures_are_independent_of_fragmentation() {
    for bin in common::bin_fixtures() {
        let input = std::fs::read(&bin).expect("fixture is readable");
        let paper = common::paper_for(&bin);
        let status = common::status_for(&bin);
        let whole = common::run_whole_in(&input, paper, status);

        for size in [1usize, 2, 3, 5, 7, 64] {
            let chunked = common::run_chunked_in(&input, &mut || size, paper, status);
            assert_eq!(whole, chunked, "{}: chunks of {size}", bin.display());
        }

        let mut rng = Rng::new(input.len() as u64 + 7);
        let random = common::run_chunked_in(&input, &mut || 1 + rng.below(16), paper, status);
        assert_eq!(whole, random, "{}: random chunks", bin.display());
    }
}
