//! Known-answer tests for the random layer.
//!
//! The [`SplitRng`] expectations come from the *reference* SplitMix64
//! algorithm, worked out independently of this code: seed 0 must produce the
//! two values every published SplitMix64 test vector lists
//! (`0xE220A8397B1DCDAF`, `0x6E789E6AA1B965F4`), and the seed-42 and split
//! sequences were computed the same way.
//!
//! The two host streams can't be exercised on the host target — they *are* the
//! ABI, and the stubs panic — so what's testable here is the pure generator,
//! the derived distributions, and the routing decision.

use wasmachine::random::{DefaultRng, Rng, SplitRng, default_random};

#[test]
fn splitmix64_reference_vectors_for_seed_zero() {
    let mut rng = SplitRng::new(0);
    assert_eq!(rng.next_u64(), 0xE220_A839_7B1D_CDAF);
    assert_eq!(rng.next_u64(), 0x6E78_9E6A_A1B9_65F4);
    assert_eq!(rng.next_u64(), 0x06C4_5D18_8009_454F);
}

#[test]
fn splitmix64_fixed_seed_gives_a_fixed_sequence() {
    let expected = [
        0xBDD7_3226_2FEB_6E95,
        0x28EF_E333_B266_F103,
        0x4752_6757_130F_9F52,
        0x581C_E1FF_0E4A_E394,
        0x09BC_585A_2448_23F2,
    ];
    let mut rng = SplitRng::new(42);
    let got: [u64; 5] = core::array::from_fn(|_| rng.next_u64());
    assert_eq!(got, expected);

    // Same seed, same sequence — the whole point.
    let mut again = SplitRng::new(42);
    assert_eq!(
        core::array::from_fn::<u64, 5, _>(|_| again.next_u64()),
        expected
    );

    // A different seed does not.
    let mut other = SplitRng::new(43);
    assert_ne!(other.next_u64(), expected[0]);
}

#[test]
fn split_produces_independent_streams() {
    // Each split consumes two draws from the parent: the child's seed and its
    // gamma. Reference values computed from the same algorithm.
    let mut parent = SplitRng::new(42);
    let mut child1 = parent.split();
    let mut child2 = parent.split();

    assert_eq!(
        [child1.next_u64(), child1.next_u64(), child1.next_u64()],
        [
            0x97C3_72BE_0195_9835,
            0x4B16_E437_27C1_D26C,
            0x1043_C9A4_AB8B_3C49
        ]
    );
    assert_eq!(
        [child2.next_u64(), child2.next_u64(), child2.next_u64()],
        [
            0x3169_7C58_6280_C6AD,
            0x9B18_20D6_E351_BDB4,
            0x9731_E945_243E_F146
        ]
    );

    // The parent picks up where its four consumed draws left off — the fifth
    // value of the plain seed-42 sequence.
    assert_eq!(parent.next_u64(), 0x09BC_585A_2448_23F2);

    // Different streams, not just different starting points: no shared values
    // across a decent window.
    let mut a = SplitRng::new(7);
    let mut b = a.split();
    let from_a: Vec<u64> = (0..64).map(|_| a.next_u64()).collect();
    let from_b: Vec<u64> = (0..64).map(|_| b.next_u64()).collect();
    assert!(from_a.iter().all(|x| !from_b.contains(x)));
}

#[test]
fn next_f64_lands_in_the_unit_interval() {
    let mut rng = SplitRng::new(1234);
    // First draw of seed 1234, hand-converted: (x >> 11) · 2⁻⁵³.
    let mut seen_low = false;
    let mut seen_high = false;
    for _ in 0..1000 {
        let f = rng.next_f64();
        assert!((0.0..1.0).contains(&f), "next_f64 gave {f}");
        seen_low |= f < 0.25;
        seen_high |= f > 0.75;
    }
    assert!(seen_low && seen_high, "next_f64 looks stuck in the middle");
}

#[test]
fn next_f64_matches_the_top_53_bits_of_next_u64() {
    // Two identically seeded streams: one read raw, one read as f64.
    let raw = SplitRng::new(99).next_u64();
    let mut f = SplitRng::new(99);
    let expected = (raw >> 11) as f64 / 9_007_199_254_740_992.0; // 2⁵³
    assert_eq!(f.next_f64(), expected);
}

#[test]
fn integer_ranges_stay_in_bounds_and_cover_them() {
    let mut rng = SplitRng::new(2024);

    // A one-element range is a constant.
    assert_eq!(rng.range(5..6), 5);
    assert_eq!(rng.range(5..=5), 5);

    let mut seen = [false; 16];
    for _ in 0..2000 {
        let n: usize = rng.range(0..16);
        assert!(n < 16);
        seen[n] = true;
    }
    assert!(seen.iter().all(|s| *s), "0..16 never produced every value");

    // Inclusive ranges reach their upper bound.
    let mut saw_six = false;
    for _ in 0..500 {
        let d: i64 = rng.range(1..=6);
        assert!((1..=6).contains(&d));
        saw_six |= d == 6;
    }
    assert!(saw_six, "1..=6 never rolled a 6");

    // Negative bounds work.
    for _ in 0..200 {
        let n: i64 = rng.range(-10..-5);
        assert!((-10..-5).contains(&n));
    }
}

#[test]
#[should_panic(expected = "empty range")]
fn empty_range_kills() {
    let mut rng = SplitRng::new(1);
    let _: i64 = rng.range(5..5);
}

#[test]
fn float_ranges() {
    let mut rng = SplitRng::new(77);
    for _ in 0..500 {
        let x: f64 = rng.range(-2.5..7.5);
        assert!((-2.5..7.5).contains(&x), "{x} left the range");
    }
}

#[test]
fn chance_at_the_extremes_is_certain() {
    let mut rng = SplitRng::new(5);
    for _ in 0..200 {
        assert!(!rng.chance(0.0));
        assert!(rng.chance(1.0));
        // Out-of-range probabilities behave like the nearest extreme.
        assert!(!rng.chance(-1.0));
        assert!(rng.chance(2.0));
    }
    // Something in between is neither always nor never.
    let mut hits = 0;
    for _ in 0..1000 {
        if rng.chance(0.5) {
            hits += 1;
        }
    }
    assert!((300..700).contains(&hits), "chance(0.5) gave {hits}/1000");
}

#[test]
fn choose_picks_from_the_slice() {
    let mut rng = SplitRng::new(11);
    let one = [42u32];
    assert_eq!(*rng.choose(&one), 42);

    let items = ['a', 'b', 'c', 'd'];
    let mut seen = [false; 4];
    for _ in 0..500 {
        let c = *rng.choose(&items);
        seen[items.iter().position(|x| *x == c).expect("in slice")] = true;
    }
    assert!(
        seen.iter().all(|s| *s),
        "choose never returned some element"
    );
}

/// Hand-traced against the published seed-0 SplitMix64 vectors, not against
/// this implementation.
///
/// Fisher-Yates walks `i = 3, 2, 1`, picking `j` uniformly in `0..=i`:
///
/// - `i = 3`, span 4. `2^64 mod 4 = 0`, so nothing is rejected and
///   `j = 0xE220A8397B1DCDAF % 4`. The low two bits of `0xAF` are `11`, so
///   `j = 3`: `swap(3, 3)` leaves `[0, 1, 2, 3]`.
/// - `i = 2`, span 3. `2^64 mod 3 = 1`, and `0x6E789E6AA1B965F4` is far above
///   that, so it is accepted. `16 ≡ 1 (mod 3)`, so the value mod 3 is the sum
///   of its hex digits mod 3 — `6+14+7+8+9+14+6+10+10+1+11+9+6+5+15+4 = 135`,
///   divisible by 3 — so `j = 0`: `swap(2, 0)` gives `[2, 1, 0, 3]`.
/// - `i = 1`, span 2. `j = 0x06C45D188009454F % 2 = 1` (the value is odd), so
///   `swap(1, 1)` changes nothing.
#[test]
fn shuffle_matches_a_hand_traced_fisher_yates() {
    let mut rng = SplitRng::new(0);
    let mut items = [0, 1, 2, 3];
    rng.shuffle(&mut items);
    assert_eq!(items, [2, 1, 0, 3]);
}

/// Three draws for four elements, none rejected above — so the stream must sit
/// exactly three values into the pinned seed-0 sequence afterwards.
#[test]
fn shuffle_consumes_one_draw_per_element_past_the_first() {
    let mut shuffled = SplitRng::new(0);
    shuffled.shuffle(&mut [0, 1, 2, 3]);

    let mut reference = SplitRng::new(0);
    for _ in 0..3 {
        let _ = reference.next_u64();
    }
    assert_eq!(shuffled, reference);
}

#[test]
fn shuffle_of_a_trivial_slice_draws_nothing() {
    // Nothing to reorder, so the stream must be untouched — a caller can shuffle
    // a possibly-empty slice without perturbing a reproducible sequence.
    let untouched = SplitRng::new(42);

    let mut rng = SplitRng::new(42);
    let mut empty: [u8; 0] = [];
    rng.shuffle(&mut empty);
    assert_eq!(rng, untouched);

    let mut one = [7u8];
    rng.shuffle(&mut one);
    assert_eq!(one, [7]);
    assert_eq!(rng, untouched);
}

#[test]
fn shuffle_is_a_permutation() {
    // Whatever the ordering, the multiset is preserved: nothing is dropped,
    // duplicated, or invented — including when elements repeat.
    let mut rng = SplitRng::new(2024);
    for len in 0..24usize {
        let original: Vec<u32> = (0..len as u32).map(|i| i % 5).collect();
        let mut items = original.clone();
        rng.shuffle(&mut items);

        let (mut a, mut b) = (original, items);
        a.sort_unstable();
        b.sort_unstable();
        assert_eq!(a, b, "shuffle of length {len} changed the multiset");
    }
}

#[test]
fn shuffle_reaches_every_permutation() {
    // All 3! orderings of three distinct elements, and nothing outside them.
    let all = [
        [1, 2, 3],
        [1, 3, 2],
        [2, 1, 3],
        [2, 3, 1],
        [3, 1, 2],
        [3, 2, 1],
    ];
    let mut seen = [0u32; 6];
    let mut rng = SplitRng::new(1234);
    for _ in 0..6000 {
        let mut items = [1, 2, 3];
        rng.shuffle(&mut items);
        let at = all.iter().position(|p| *p == items).expect("a permutation");
        seen[at] += 1;
    }
    assert!(
        seen.iter().all(|c| (600..1400).contains(c)),
        "permutations came out lopsided: {seen:?}"
    );
}

#[test]
fn shuffle_actually_reorders_a_long_slice() {
    // 32! orderings; landing on the identity would be a broken shuffle, not luck.
    let mut rng = SplitRng::new(9);
    let mut items: Vec<u32> = (0..32).collect();
    rng.shuffle(&mut items);
    assert_ne!(items, (0..32).collect::<Vec<u32>>());

    // And it is reproducible from the seed, like every other draw here.
    let mut again: Vec<u32> = (0..32).collect();
    SplitRng::new(9).shuffle(&mut again);
    assert_eq!(again, items);
}

#[test]
#[should_panic(expected = "empty slice")]
fn choose_on_an_empty_slice_kills() {
    let mut rng = SplitRng::new(1);
    let empty: [u8; 0] = [];
    let _ = rng.choose(&empty);
}

#[test]
fn rng_is_usable_behind_a_mutable_reference() {
    // SDK APIs take `&mut impl Rng`; passing one along must not need a
    // reborrow dance at every level.
    fn draw(rng: &mut impl Rng) -> u64 {
        rng.next_u64()
    }
    fn forward(mut rng: impl Rng) -> u64 {
        draw(&mut rng)
    }
    let mut rng = SplitRng::new(0);
    assert_eq!(draw(&mut rng), 0xE220_A839_7B1D_CDAF);
    assert_eq!(forward(&mut SplitRng::new(0)), 0xE220_A839_7B1D_CDAF);
}

#[test]
fn splitrng_survives_a_byte_round_trip() {
    // Pod is what lets a stream be sent through a channel.
    let mut rng = SplitRng::new(42);
    let _ = rng.next_u64();
    let bytes = bytemuck::bytes_of(&rng).to_vec();
    assert_eq!(bytes.len(), 16);
    let mut restored: SplitRng = *bytemuck::from_bytes(&bytes);
    assert_eq!(restored, rng);
    // And it continues the same sequence.
    assert_eq!(restored.next_u64(), 0x28EF_E333_B266_F103);
}

/// Without a `random_seed = N` on the SDK's `main` attribute, `default_random`
/// must route
/// to the non-deterministic host stream. (The seeded route sets a
/// write-once global during macro-generated init, which no host test can do
/// without the ABI; the Phase-3 integration test drives it through real wasm.)
#[test]
fn default_random_routes_to_the_nondeterministic_stream_when_unseeded() {
    assert!(matches!(default_random(), DefaultRng::NonDeterministic(_)));
}

/// Regression: a signed range wider than its own type's positive half.
///
/// The span has to be computed in same-width *unsigned* arithmetic. Computing it
/// as a signed subtraction first wraps (`2_000_000_000i32 - (-2_000_000_000)` is
/// `-294_967_296`) and widening *that* to `u64` sign-extends it into a span of
/// ~1.8e19, so draws landed far outside the range — this test caught
/// `2_083_071_605` from a range ending at `2_000_000_000`.
#[test]
fn wide_signed_ranges_stay_in_bounds() {
    let mut rng = SplitRng::new(42);
    for _ in 0..2000 {
        let n: i32 = rng.range(-2_000_000_000..2_000_000_000);
        assert!(
            (-2_000_000_000..2_000_000_000).contains(&n),
            "draw {n} left -2_000_000_000..2_000_000_000"
        );
    }

    // The extremes: the whole type, exclusive and inclusive.
    let mut rng = SplitRng::new(7);
    for _ in 0..2000 {
        let n: i32 = rng.range(i32::MIN..i32::MAX);
        assert!(
            (i32::MIN..i32::MAX).contains(&n),
            "draw {n} left i32::MIN..i32::MAX"
        );
    }
    let mut rng = SplitRng::new(7);
    for _ in 0..2000 {
        // Span 2^32: one more than fits in a u32, so it has to be widened to
        // u64 *after* the unsigned subtraction. Every i32 is in range, so there
        // is nothing to assert beyond "this does not panic or wrap wrongly".
        let _: i32 = rng.range(i32::MIN..=i32::MAX);
    }

    // The same shape one width up, where the span nearly fills u64.
    let mut rng = SplitRng::new(11);
    for _ in 0..2000 {
        let n: i64 = rng.range(-4_000_000_000_000_000_000..4_000_000_000_000_000_000);
        assert!(
            (-4_000_000_000_000_000_000..4_000_000_000_000_000_000).contains(&n),
            "draw {n} left the i64 range"
        );
    }

    // Unsigned narrow ranges were never affected, but they share the code path.
    let mut rng = SplitRng::new(3);
    for _ in 0..2000 {
        let n: u32 = rng.range(0..u32::MAX);
        assert!(n < u32::MAX);
    }
}

/// The exact first draws of those ranges, worked out from the reference
/// SplitMix64 sequence and the documented rejection rule (reject the first
/// `2^64 mod span` values, then take `x % span`) — never read off this code.
#[test]
fn wide_range_draws_match_the_reference_sequence() {
    // Seed 42, span 4_000_000_000. `2^64 mod span` = 3_709_551_616; the first
    // draw 0xBDD732262FEB6E95 is above that, and `% span` = 755_275_413, so
    // -2_000_000_000 + 755_275_413 = -1_244_724_587.
    let mut rng = SplitRng::new(42);
    assert_eq!(rng.range(-2_000_000_000..2_000_000_000), -1_244_724_587i32);

    // Seed 7, span 0xFFFF_FFFF: the first accepted draw `% span` is
    // 3_170_758_587, and i32::MIN + 3_170_758_587 = 1_023_274_939.
    let mut rng = SplitRng::new(7);
    assert_eq!(rng.range(i32::MIN..i32::MAX), 1_023_274_939i32);

    // Seed 7 inclusive: span is exactly 2^32, so nothing is rejected and the
    // answer is the low 32 bits of the first draw (0x63CBE1E459320DD7 →
    // 0x59320DD7 = 1_496_486_871) offset from i32::MIN: -651_031_081.
    let mut rng = SplitRng::new(7);
    assert_eq!(rng.range(i32::MIN..=i32::MAX), -651_031_081i32);
}
