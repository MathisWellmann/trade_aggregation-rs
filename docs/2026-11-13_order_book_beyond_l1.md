# Design for order book data beyond L1 (2026-11-13)

Status: proposal, not implemented. This document lists a design and the
reasons for it. It does not change any code. It is the follow-up promised by
the v14 plan ("Order book depth: see step 9" / "Not done: order book depth").

## Goal

`BestQuote` (`src/market_event.rs`) gives only the top of book: one bid
price and size, one ask price and size. Several components that the
market-microstructure literature finds useful need more than the top level.
This document lays out what L2 (aggregated price levels) and L3
(individual orders) data would unlock, how to accept it without the crate
reconstructing a book itself, which components and rules it would enable,
and a survey of existing Rust crates for book reconstruction, so a future
implementation has a grounded starting point.

## What L2/L3 unlock beyond L1

L1 cannot see depth beyond the best price, so it cannot see how much flow
happened within a given distance of the mid, how steep the book is, what it
would cost to fill a given size, or how a shock to liquidity recovers. L2
and L3 unlock, respectively:

- **L2 (price levels, size per level, no order identity):**
  multi-level order-flow imbalance, depth imbalance across levels,
  cumulative depth within a bps band, book slope, and cost-to-trade for a
  hypothetical order size.
- **L3 (individual orders, with identity):** everything L2 gives, plus
  queue position and queue depletion/resiliency (time for depth to recover
  after a shock), and separating order adds from cancels from trade-throughs
  at the order level rather than only at the net-size-per-level level.

Three findings from the literature matter for how this crate should expose
any of it:

- Xu, Gould and Howison (2019) show that including order flow from deeper
  levels, not just the best level, meaningfully improves the out-of-sample
  fit between order flow and the contemporaneous mid-price change (15-75%
  RMSE reduction depending on tick size), but that the ten per-level flow
  numbers are highly correlated with each other, so a plain multi-variable
  regression on them overfits; a regularized fit (they use ridge
  regression) is needed to use them directly.
- Cont, Cucuringu and Zhang (2023) show that the first principal component
  of the multi-level order-flow-imbalance vector explains over 89% of its
  cross-level variance, and that this single, PCA-weighted "integrated OFI"
  scalar explains contemporaneous price impact about as well as, or better
  than, the raw multi-level vector, while being one number instead of ten.
  Once that scalar is used, cross-asset terms stop adding explanatory power
  for the *contemporaneous* price move (though lagged cross-asset order
  flow still helps short-horizon forecasting).
- Kolm, Turiel and Westray (2023) show that deep-learning models trained on
  stationary, order-flow-*derived* features significantly outperform models
  trained directly on raw order-book snapshots. This validates the
  design this crate already follows for L1 (derive a small number of
  scale-free `CandleComponent`s, never expose raw book state as a field)
  and is the main reason to extend that same approach to L2/L3 rather than
  to add a raw-depth-snapshot component.

## Book-agnostic event design

The crate should not reconstruct a book itself. This is already the stated
boundary for order book data ("A full book is the wrong state for a candle
component. A caller can send summaries of the book as its own event type.",
v14 plan, step 9) and this proposal keeps it: reconstruction from exchange
snapshots and diffs stays the caller's job, same as merging and reordering
several input streams already is. The crate's job is to accept an
already-reconstructed view of the book through a trait.

### Two ways to wire it into the event model

1. **A third `MarketEvent` variant.** Extend `MarketEvent<T, Q>`
   (`src/market_event.rs`) to `MarketEvent<T, Q, D>` with a `Depth(D)`
   case alongside `Trade(T)` and `Quote(Q)`. This reuses the existing
   `AsTrade`/`AsQuote`-style dispatch and the single time-ordered stream
   model that already lets trades and quotes share one aggregator. The cost
   is that it changes the arity of a type that is already in public use:
   the blanket `AsTrade for T`, `AsQuote for Q`, and the `same_resolution`
   check between exactly two event kinds (`src/market_event.rs`) would all need a third `AsDepth` projection and a third
   resolution check, and every existing caller of `MarketEvent<T, Q>` would
   need to pick a `D` even if that symbol never carries depth data.
2. **A separate trait, no change to `MarketEvent<T, Q>`.** Define
   `BookLevels` (shaped like `BestQuote`) as the trait the crate asks for,
   and `AsDepth` (shaped like `AsQuote`) as the projection trait. A caller
   who wants trades, quotes and depth together defines their own three-way
   event enum (a few lines, the same amount of code `Quote` already costs
   today as a concrete `BestQuote` implementation) and implements
   `AsTrade`/`AsQuote`/`AsDepth` on it. `MarketEvent<T, Q>` is untouched; a
   `MarketEvent3<T, Q, D>` convenience type could be added later, purely
   additively, once the two-variant type has had time to prove the pattern.

Recommendation: prefer option 2. The amount of code that already depends on
`MarketEvent<T, Q>`'s exact arity (confirmed by reading
`src/market_event.rs`) makes a breaking three-way redesign expensive for a
feature most callers will not use, while the narrow-trait approach costs
callers who do want depth only a small, local type, matching the existing
"a plain trade type keeps working without a wrapper" precedent.

### Shape of `BookLevels` / `DepthSnapshot`

Model it as an ordered view, best price first, not a fixed-size array or a
`Vec`: `bids(&self) -> impl Iterator<Item = (f64, f64)>` and `asks(&self)
-> impl Iterator<Item = (f64, f64)>` (price, size). L2 feeds commonly carry
5, 10, 20 or full depth, and a component that only needs, say, cumulative
depth within 10 bps should be able to stop iterating once it has moved past
that band, without the crate allocating a buffer for depth it never uses.
`DepthSnapshot` is this same shape; the two names in the original ask refer
to the same concept (a reconstructed book at a point in time), so this
design proposes one trait, `BookLevels`, for it.

For L3 detail (queue position, add/cancel counts), the crate does not need
to model individual orders itself: those counts are naturally derived
caller-side from an order-level feed, and handed to the crate as plain
numeric fields on a custom event (the same way `Quote`, a concrete
`BestQuote`, is a convenience struct for the common L1 case while the trait
stays general). A `BookLevels` implementation plus a couple of counters is
enough for every component and rule listed below.

## Components enabled

- **Multi-level OFI.** Ship the OFI of each of the first M levels
  (Xu, Gould and Howison) as the main component: one value per level, e.g.
  M flat components or one component per level index as a const generic.
  The integrated OFI of Cont, Cucuringu and Zhang is a weighted sum of these
  levels, with weights from a principal component analysis fitted on past
  data. The crate cannot know these weights, and a component cannot take
  them at runtime (components are built with `Default`), so the caller
  applies the projection downstream, fitted on a past window only to avoid
  lookahead. The component keeps the previous top-M levels (a fixed array)
  to compute the per-level flow of the next update, like
  `OrderFlowImbalance` keeps the previous best quote.
- **Depth imbalance across levels.** The multi-level generalization of
  `TimeWeightedImbalance`: sum bid size and ask size over the first M
  levels, instead of just the best one, before taking the imbalance ratio.
- **Cumulative depth within X bps of mid.** Walk `BookLevels` from the best
  price, summing size, until the price has moved X bps from the mid; a
  direct use of the ordered-iterator shape above.
- **Book slope.** A cheap proxy for the price-impact curve's steepness,
  e.g. `(depth at level M - depth at level 1) / (price at level M - price
  at level 1)`, or a regression fit across levels for a smoother estimate.
- **Cost-to-trade (walk-the-book VWAP).** For a hypothetical order of size
  Q, walk `BookLevels` consuming levels until Q is filled and return the
  size-weighted average price. This needs only the current snapshot, no
  running state across events, so it is the cheapest of these components
  to add correctly.
- **Queue depletion / resiliency.** Needs L3: measure the time between a
  liquidity shock (a large, sudden depth removal) and depth recovering to
  its pre-shock level. Unlike cost-to-trade, this is path-dependent: it
  needs a small state machine ("a shock has been in progress since ts, at
  pre-shock depth X"), not just the current snapshot.
- **Order add/cancel rates.** Needs L3 event counts (adds, cancels, trades)
  per unit of candle time. These are plain counters, exactly mergeable by
  summation, the same monoid argument as `NumTrades` and `QuoteUpdates` in
  the companion N x M x K document.

## Rules enabled

- **Multi-level / integrated OFI bars.** Close when the accumulated
  integrated OFI since the candle opened crosses a threshold; the same
  accumulate-a-sum-since-open shape as the existing `OrderFlowImbalance`
  quote component (`src/candle_components/quotes.rs`), but driven
  by depth deltas instead of L1 quote deltas, and usable either as a
  component (report the value) or as an `AggregationRule` (trigger on it),
  mirroring how `SignedVolumeRule` is the rule-shaped counterpart of a
  volume-flow component.
- **Depth-normalised OFI bars.** The level-1 version exists:
  `OfiRule` with `OfiNormalization::Depth` divides each OFI update by the
  depth of the previous best quote. With book levels, the same rule can sum
  the flow and the depth over the first M levels. Normalize OFI by the
  resting depth before thresholding, so the same raw order flow means more in a thin book
  and less in a deep one. This addresses the same instability already
  documented for the existing imbalance and run bars (README: "the expected
  imbalance goes to zero and bars collapse to single trades" in a balanced,
  deep market); the existing `Adaptive` recalibration mechanism
  (`src/aggregation_rules/adaptive.rs`) is a plausible fit for calibrating a
  depth-normalised threshold too, though that is a separate design
  decision from this one.
- **Liquidity-shock bars.** Close a candle when depth within X bps drops by
  more than a threshold fraction within a short window. This is
  path-dependent, like CUSUM, range and directional-change bars in the
  companion document: it keeps a small extent/shock state rather than a
  counter against a fixed threshold, so the same "does not nest across
  parameters" caveat applies to a bank of liquidity-shock bars at different
  thresholds.
- **Book-imbalance regime bars.** Close when the (depth- or L1-) imbalance
  crosses zero, or stays beyond a band for a sustained period; conceptually
  the same trend-state machine as `DirectionalChangeRule`
  (`src/aggregation_rules/directional_change_rule.rs`), but tracking
  imbalance instead of price.

## Survey of Rust order book reconstruction crates

Checked via the crates.io API and, for the top candidates, their READMEs
and GitHub activity (accessed 2026-11-13).

| Crate | Downloads | License | Latest release | Last activity | L2/L3 | Price repr. | Fit |
|---|---|---|---|---|---|---|---|
| `orderbook-rs` | 134,960 | MIT | v0.16.0 | 2026-11-13 (same day) | L2 building blocks (lock-free price levels) | `f64` | Narrow, actively released; best fit as an optional dependency |
| `pricelevel` | 169,674 | MIT | v0.10.2 | 2026-09-30 | companion to `orderbook-rs` (one matcher per level, atomic counters) | `f64` | Same author/scope as above |
| `hftbacktest` | 25,630 | MIT | v0.9.4 | 2025-12-23 (repo push), 5,083 GitHub stars | Full L2 (MBP) + L3 (MBO) reconstruction, exchange connectors (Binance Futures, Bybit) | configurable | Full backtesting framework, not a narrow library; better as a caller-side tool than a dependency |
| `bourse-book` | 10,444 | MIT | - | 2024-03-28 (stale) | simulated LOB | `f64` | Not for live reconstruction |
| `lobster` | 24,594 | ISC | - | 2020-10-26 (stale, 5+ years) | L2/L3 modeling | - | Unmaintained |
| `limitbook` | 962 | Apache-2.0 | v0.1.0 | 2025-08-08 | L2 | - | Single release, unproven |
| `book-kit` | 415 | MIT/Apache-2.0 | v0.1.1 | 2026-10-24 | L2, "loom-verified" | - | Very new, low adoption |
| `depthbook` | 42 | dual | v0.7.0 | 2026-09-19 | fixed-capacity sorted sides, sequence-gap detection | scale-9 fixed-point integers | 0 GitHub stars, essentially unused, but notable for an integer price representation |
| `dlob` | low | GPL-3.0-only | - | - | L2/L3 | - | License excludes it regardless of technical fit (see below) |

Negative finding: no crate found is both well-maintained and narrowly
scoped to "reconstruct an L2/L3 book from exchange snapshots and diffs" as a
lightweight dependency. The two actively maintained, well-adopted options
sit at opposite ends: `orderbook-rs`/`pricelevel` are low-level building
blocks from the same author, narrowly scoped, MIT-licensed, and released as
recently as the day this document was written; `hftbacktest` is a complete,
opinionated backtesting framework with its own connectors and Python
bindings, a fit as a tool a caller runs this crate inside of, not as a
dependency this crate should pull in.

`depthbook`'s fixed-point (scale-9 integer) price representation is worth
noting even though the crate itself is unused: it is the same tradeoff this
crate's own "generic over the price type" TODO (README) will eventually
need to weigh for order book levels, where summing many small sizes across
levels makes floating-point drift more visible than it is for a single
trade price.

`dlob` is excluded regardless of its technical merit: it is GPL-3.0-only,
and this crate (AGPLv3 plus a commercial license option, per its README)
should not take a GPL-3.0-only dependency, since that would narrow the
commercial licensing path this crate already offers.

## Recommendation and integration boundary

Keep this crate's dependency surface at zero for book reconstruction by
default. This matches the already-stated principle that a full book is the
wrong state for a candle component, and that reconstruction is a caller
concern. Concretely:

- Ship the `BookLevels`/`AsDepth` traits and the depth-derived
  `CandleComponent`s and `AggregationRule`s from this document as the
  crate's contribution; they depend on nothing but the trait.
- If a convenience adapter is ever wanted, make it an optional,
  off-by-default Cargo feature that depends on `orderbook-rs`/`pricelevel`
  (same author, MIT, narrowly scoped, actively released) to translate that
  crate's book type into this crate's `BookLevels` trait, verified by a
  conformance test rather than assumed to be correct.
- Never depend on `hftbacktest`: it is a framework a caller would run this
  crate inside of, not a library this crate should embed.

This is not a commitment to build the adapter; it only records which
direction an adapter should go if one is ever built.

## Sources

- Xu, K., Gould, M. D., & Howison, S. D. (2019). Multi-Level Order-Flow
  Imbalance in a Limit Order Book. *Market Microstructure and Liquidity*,
  4(3-4), 1950011. https://doi.org/10.1142/S2382626619500114,
  arXiv:1907.06230.
- Cont, R., Cucuringu, M., & Zhang, C. (2023). Cross-impact of order flow
  imbalance in equity markets. *Quantitative Finance*, 23(10), 1373-1393.
  https://doi.org/10.1080/14697688.2023.2236159.
- Kolm, P. N., Turiel, J., & Westray, N. (2023). Deep order flow imbalance:
  Extracting alpha at multiple horizons from the limit order book.
  *Mathematical Finance*, 33(4), 1044-1081.
  https://doi.org/10.1111/mafi.12413.
- crates.io listings and READMEs for `orderbook-rs`, `pricelevel`,
  `hftbacktest`, `bourse-book`, `lobster`, `limitbook`, `book-kit`,
  `depthbook` and `dlob` (accessed 2026-11-13).

## Not in this proposal

- Order book reconstruction itself: left to the caller, or to an optional,
  not-yet-built adapter feature.
- Any implementation of the traits, components or rules above; this
  document is a design only.
- The N x M x K architecture discussed in the companion document
  (`2026-11-13_nxmxk_architecture.md`): that document covers running many
  symbols and many rule parameters, independently of order book depth.
- The level-1 work that this document builds on (price sources, `OfiRule`,
  `QuoteOfi`, the level-1 quote components) is implemented in the same
  series of revisions; nothing here changes it.
