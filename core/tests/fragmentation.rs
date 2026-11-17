//! Fragmented input must give exactly the same result as the whole input.

mod common;

use common::Rng;

#[test]
fn fixtures_are_independent_of_fragmentation() {
    for bin in common::bin_fixtures() {
        let input = std::fs::read(&bin).expect("fixture is readable");
        let paper = common::paper_for(&bin);
        let whole = common::run_whole(&input, paper);

        for size in [1usize, 2, 3, 5, 7, 64] {
            let chunked = common::run_chunked(&input, &mut || size, paper);
            assert_eq!(whole, chunked, "{}: chunks of {size}", bin.display());
        }

        let mut rng = Rng::new(input.len() as u64 + 7);
        let random = common::run_chunked(&input, &mut || 1 + rng.below(16), paper);
        assert_eq!(whole, random, "{}: random chunks", bin.display());
    }
}
