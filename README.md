# Trade Aggregation
A high performance, modular and flexible trade aggregation crate, producing Candle data, 
suitable for low-latency applications and incremental updates.
It allows the user to choose the rule dictating how a new candle is created 
through the [AggregationRule](src/aggregation_rules/aggregation_rule_trait.rs) trait, 
e.g: Time, Volume based or some other information driven rule.
It also allows the user to choose which type of candle will be created from the aggregation process
through the [ModularCandle](src/modular_candle_trait.rs) trait. Combined with the [Candle](trade_aggregation_derive/src/lib.rs) macro, 
it enables the user to flexibly create any type of Candle as long as each component implements 
the [CandleComponent](src/candle_components/candle_component_trait.rs) trait.
The aggregation process is also generic over the type of input events,
e.g. trades that implement the [TakerTrade](src/types.rs) trait, 
allowing for greater flexibility for downstream projects.
An event type states the unit of its timestamps (`Timestamped::TIMESTAMP_RESOLUTION`)
and a trade type the currency of its sizes (`TakerTrade::SIZE_UNIT`, base or quote),
so that `By::Base` and `By::Quote` sum volumes and notionals correctly for spot, linear and inverse markets.

See [MathisWellmann/go_trade_aggregation](https://github.com/MathisWellmann/go_trade_aggregation) for a go implementation with less features and performance.

Here is a sample trade series of `Bitmex_XBTUSD` aggregated into 15 minute candles:
![time_candles](readme_img/time_candles_plot.png)

## Features:
### `AggregationRule`:
The pre-existing rules in this crate include:
'AggregationRule'   | Description
--------------------|-------------
`AlignedTimeRule`   | Create candles every period, aligned to the epoch (plus an optional offset)
`TimeRule`          | Create candles every period, counted from the first event
`VolumeRule`        | Create candles every n units traded
`TickRule`          | Create candles every n ticks
`RelativePriceRule` | Create candles with every n basis points price movement (Renko)
`RangeRule`         | Create range bars: close a candle when its (high - low) / open reaches n basis points
`DirectionalChangeRule` | Close a candle at every directional change: a reversal of n basis points from the last extreme ("intrinsic time")
`IntrinsicEventRule` | Close a candle at every directional change and at every overshoot of n basis points beyond the last event (the full intrinsic time of Glattfelder, Dupuis and Olsen)
`CusumRule`         | Close a candle when a symmetric CUSUM filter on a signal, the log returns by default, reaches a threshold (a drift in one direction)
`VolatilityRule`    | Create volatility bars: close a candle when the sum of absolute log returns reaches n basis points
`VarianceRule`      | Close a candle when the sum of squared log returns reaches a threshold: a clock of the quadratic variation ("business time"), so the returns of the candles are close to Gaussian
`JumpRule`          | Close a candle at a return larger than k jump-robust local standard deviations (Lee and Mykland), e.g. combined with another rule so a jump starts a new candle
`PriceChangeRule`   | Close a candle every n changes of the price, e.g. of the mid price ("mid-change bars")
`SignedVolumeRule`  | Create order flow ("delta") bars: close a candle when the net taker volume, buys minus sells, reaches n units
`OfiRule`           | Create quote delta bars: close a candle when the order flow imbalance of the quotes (Cont, Kukanov and Stoikov), raw or divided by the depth, reaches a threshold
`ImbalanceRule`     | Create tick, volume or dollar imbalance bars (López de Prado): close a candle when the net order flow exceeds the expected bar length times the expected imbalance per trade
`RunRule`           | Create tick, volume or dollar run bars (López de Prado): close a candle when one side's flow exceeds the expected bar length times the expected flow of that side
`ActivityBurstRule` | Close a candle where the trade arrival regime changes: when a fast intensity of the trades rises above, or falls back towards, a slow one
`SurprisalRule`     | Experimental information bars: close a candle after a given number of bits of surprise in the trade sides, under an online model of the order flow

### Price sources
Trade prices alternate between the bid and the ask ("bid-ask bounce"),
so sums of absolute or squared trade returns grow with the number of trades even when the market does not move.
The price driven rules (`RelativePriceRule`, `RangeRule`, `DirectionalChangeRule`, `IntrinsicEventRule`, `VolatilityRule`,
`VarianceRule`, `JumpRule`, `PriceChangeRule`, and `CusumRule` through `LogReturn`)
and the return components follow a `PriceSource` of your choice:
`TradePrice` (the default), `MidPrice` or `Microprice` (the bid and the ask weighted by the size on the other side,
the first order of the micro-price of Stoikov) of the quotes of a `MarketEvent` stream.
Choose it at compile time with `with_price_source` on a rule or the type parameter of a component, e.g. `RealizedVariance<MidPrice>`,
or at runtime with `PriceKind`. A quote with a non-positive or crossed bid and ask has no price.

`CusumRule` filters any `Signal`: `LogReturn` of a price source, `SignedVolume`, `QuoteOfi` or `LogInterArrival`.
`Standardized` turns a signal into z-scores with an exponentially weighted mean and variance from before each value,
so that `CusumRule::new(3.0)?.with_signal(Standardized::new(LogReturn::new(MidPrice), alpha, warmup)?)`
fires at a drift of three standard deviations, and one parameter fits every symbol.

```rust
use trade_aggregation::{
    candle_components::{RealizedVariance, TotalVariation},
    *,
};

#[derive(Debug, Default, Clone, Candle)]
struct MyCandle {
    rv_trades: RealizedVariance,
    rv_mid: RealizedVariance<MidPrice>,
    path_microprice: TotalVariation<Microprice>,
}

# fn main() -> trade_aggregation::Result<()> {
// Volatility bars on the mid price, which does not bounce between the bid and the ask.
let rule = VolatilityRule::new(0.01)?.with_price_source(MidPrice);
let mut aggregator = GenericAggregator::<MyCandle, _, MarketEvent<Trade, Quote>>::new(rule, false);
let mut closed = Vec::new();
let quote = Quote { timestamp: 0, bid_price: 99.0, bid_size: 1.0, ask_price: 101.0, ask_size: 1.0 };
aggregator.update(&MarketEvent::Quote(quote), &mut closed);
for (timestamp, price) in [(1, 101.0), (2, 99.0), (3, 101.0)] {
    let trade = Trade { timestamp, price, size: 1.0 };
    aggregator.update(&MarketEvent::Trade(trade), &mut closed);
}
// The trades bounce, the mid price does not move.
assert!(aggregator.unfinished_candle().rv_trades() > 0.0);
assert_eq!(aggregator.unfinished_candle().rv_mid(), 0.0);

// A CUSUM filter at a drift of three standard deviations of the microprice returns.
let _cusum = CusumRule::new(3.0)?.with_signal(Standardized::new(LogReturn::new(Microprice), 0.01, 100)?);
# Ok(())
# }
```

### Combinators
Rules can be combined:
'Combinator'          | Description
----------------------|-------------
`AnyRule<A, B>`       | Close the candle when either rule triggers, e.g. dollar volume or volatility, whichever comes first ("hybrid bars")
`AllRule<A, B>`       | Close the candle once both rules have triggered since it opened
`MinDuration<R>`      | Keep every candle open for at least a given period, so bursts do not create a flood of tiny candles
`MaxDuration<R>`      | Close a candle a given period after it opened, so quiet markets still produce candles

`AnyRule::last_trigger` and `AllRule::last_trigger` tell which inner rule closed the last candle (`Fired::First`, `Second` or `Both`):
which criterion closed a hybrid bar is a feature itself, and the share of candles closed by a timeout shows a badly calibrated threshold.
`first()`, `second()` and `inner()` give access to the inner rules, e.g. to the last event of an `IntrinsicEventRule`.

`Adaptive<R>` recalibrates the threshold of a rule (any rule that implements `ThresholdRule`) after each of its candles,
so that candles last a target duration on average whatever the activity of the market.
A fixed threshold drifts with the regime: a candle of 20000 trades can last two hours in a quiet year and three minutes in a busy one.
`Adaptive` keeps an exponentially weighted moving average of the duration per unit of activity
and sets the threshold to the activity of the target duration, clamped to given bounds.
This aims at the arithmetic mean duration, e.g. about 1440 candles per day for a target of one minute.
`with_scaling(Scaling::Diffusive)` is for rules whose activity grows with the square root of time,
such as the net flow of a `SignedVolumeRule` or a price move ("adaptive delta bars").
Combine it with a duration bound as `MaxDuration<Adaptive<R>>`.

The imbalance and run bars learn their threshold from the market, which is known to be unstable:
with balanced flow the expected imbalance goes to zero and bars collapse to single trades,
and a learned bar length tends to run to its bounds.
`FlowBarParams` therefore keeps the expected bar length fixed by default, floors the expected imbalance,
and bounds a learned bar length; `Adaptive` can calibrate the expected bar length to a target duration instead.

When a combinator closes a candle that an inner rule did not trigger, it calls `AggregationRule::reset` on that rule,
so that every rule starts the next candle from scratch.

### Deadlines, watermarks and gaps
Time rules and `MaxDuration` have a deadline: the time at which they close a candle without another event.
The `GenericAggregator` closes the candle at its deadline as soon as an event or a watermark reaches it,
and stamps the candle with that time:

- `update(event, &mut out)` first closes the candles whose deadline passed before the event,
  so an event never goes into a period that ended before it.
- `advance_to(watermark, &mut out)` closes them without an event, e.g. on a heartbeat of a live stream.
  The candles are the same whether or when heartbeats come, so a live stream and a replay of it produce the same candles.
- `with_gaps(Gaps::Flat { max_candles })` emits an empty candle (flat at the previous close) for each period without events,
  up to `max_candles` in a row; `Gaps::Skip` (the default) emits nothing.
- `force_close(timestamp, &mut out)` closes the current candle whatever the rule says, as `BarKind::Forced`.
  It is for a market clock: a coordinator runs one rule on a leader symbol or on the notional of the whole market,
  and force-closes the aggregators of all symbols at the same time, so a cross-sectional model gets one row per symbol per tick.

Each closed candle comes in a `Bar`, with a sequence number, the open and close time, the number of events and why it closed.
`provisional(now)` returns the unfinished candle the same way, e.g. for features over a window that ends now.
A fixed boundary, e.g. every UTC midnight, is `AnyRule::new(rule, AlignedTimeRule::new(D1))`.

If these don't satisfy your desires, just create your own by implementing the [`AggregationRule`](src/aggregation_rules/aggregation_rule_trait.rs) trait,
and you can plug and play it into the [`GenericAggregator`](src/aggregator.rs).
Implement `reset` as well if your rule accumulates something per candle and should work inside the combinators,
and `deadline`, `on_deadline` and `skip_to` if it closes candles on a clock.

### Trades and quotes in one candle
`MarketEvent<T, Q>` is either a trade `T` (a `TakerTrade`) or a quote `Q` (a `BestQuote`: best bid and ask),
so that one aggregator can process both streams of a symbol in time order.
Trade rules and trade components see only the trades (through `AsTrade`), quote components only the quotes (`AsQuote`),
and time rules every event. A plain trade type keeps working without a wrapper.
Both types must have the same timestamp resolution, which is checked at compile time.

### `CandleComponent`:
These pre-existing 'CandleComponents' exist out of the box.
A value that is not defined for a candle, e.g. a ratio of an empty candle, is `None`, never `NaN`.
Statistics that are sums ("additive") add up exactly over consecutive candles,
so a model can compute them over any window of candles without aggregating again.

Prices and volumes:
'CandleComponent'   | Description
--------------------| ---
`Open`, `High`, `Low`, `Close` | The trade prices at the beginning, the extremes and the end of a candle
`Volume`            | The cumulative trading volume, in the unit of the trade sizes
`BaseVolume`        | The cumulative trading volume in the base currency
`QuoteVolume`       | The cumulative trading volume in the quote currency (the notional)
`VolumeBuys`, `VolumeSells` | The volume of the taker buys and of the taker sells
`NumTrades`         | The number of trades during the candle
`AveragePrice`      | The equally weighted average price
`WeightedPrice`     | The volume weighted price
`MedianPrice`       | The exact median trade price (streaming, with two heaps)
`ApproxMedianPrice` | The median trade price in constant memory (the P² algorithm of Jain and Chlamtac)
`StdDevPrices`, `StdDevSizes` | The standard deviation of the prices and of the sizes, in raw units
`RelativeStdDevPrices`, `RelativeStdDevSizes` | The standard deviation divided by the mean, comparable across symbols
`TimeVelocity`      | Essentially how fast the candle was created time wise
`Trades`            | Just returns the observed trades during that candle

Return path, for any price source (`<P = TradePrice>`); each return starts at the last price of the previous candle:
'CandleComponent'   | Description
--------------------| ---
`RealizedVariance`  | The sum of the squared log returns
`BipowerVariation`  | The jump-robust bipower variation of the log returns
`RealizedQuarticity`, `RealizedQuartic` | The realized quarticity and the raw sum of the fourth powers of the log returns
`PositiveSemivariance`, `NegativeSemivariance` | The squared log returns of the up and of the down moves (Barndorff-Nielsen, Kinnebrock and Shephard; Patton and Sheppard)
`RealizedCubic`     | The sum of the cubed log returns; `realized_skewness` and `realized_kurtosis` turn the sums into moments
`ReturnCount`, `UpTicks`, `DownTicks`, `UnchangedTicks` | The number of returns, and of the positive, negative and zero ones
`TotalVariation`    | The sum of the absolute log returns (the "coastline")
`MaxAbsReturn`      | The largest absolute log return, which indicates a jump
`EfficiencyRatio`   | The net move divided by the path length, from 0 (chop) to 1 (a straight trend)
`ReturnAutocovariance` | The sum of the products of neighbouring returns; `roll_spread` turns it into the spread estimate of Roll
`MaxDrawdown`, `MaxRunup` | The largest fall from a running peak and rise from a running trough of the log price
`TimeOfHigh`, `TimeOfLow` | When in the candle the high and the low happened, as a fraction of its duration
`TwoScaleRealizedVariance` | The realized variance that is robust to microstructure noise (Zhang, Mykland and Aït-Sahalia)
`WapRealizedVariance` | `RealizedVariance<Microprice>`

Order flow and trades:
'CandleComponent'   | Description
--------------------| ---
`KyleLambda`, `KyleLambdaStats` | The price impact per signed notional (Kyle), and its additive regression sums
`BuyVwap`, `SellVwap` | The volume weighted price of the taker buys and of the taker sells
`NetFlow`, `NetFlowHigh`, `NetFlowLow` | The net taker volume and its highest and lowest running value in the candle
`DirectionalTradeRatio`, `DirectionalVolumeRatio` | The share of buys in the trades and in the volume
`Entropy`           | Binary Shannon entropy using the trade side as inputs
`ConditionalSignEntropy` | The entropy of a trade side given the previous side
`SignSwitches`, `LongestRun`, `SignAutocorrelation` | The persistence of the trade sides (Lillo and Farmer)
`LargestTrade`      | The size of the largest trade in the base currency
`LargestTradeShare`, `SizeHerfindahl` | The share of the largest trade in the volume, and the concentration of the trade sizes
`SmallTradeFlow`, `MediumTradeFlow`, `LargeTradeFlow` | The net volume of small, medium and large trades, relative to the recent average size
`TakerOrders`, `Sweeps`, `MaxSweepDepth` | The taker orders (fills with the same time and side), how many swept several prices, and how far
`MaxGap`, `InterArrivalCv` | The longest time between trades, and the burstiness of the times between trades

Quotes (best bid and ask):
'CandleComponent'   | Description
--------------------| ---
`MidOpen`, `MidHigh`, `MidLow`, `MidClose` | The mid price path of the quotes, starting at the mid in effect
`QuoteUpdates`      | The number of quote updates
`TimeWeightedSpread` | The relative bid-ask spread, averaged over the time of the candle
`TimeWeightedImbalance` | The level 1 size imbalance, averaged over the time of the candle
`TimeWeightedDepth` | The size at the best bid and ask, averaged over the time of the candle
`SpreadAtClose`, `ImbalanceAtClose`, `MicropriceOffsetAtClose`, `DepthAtClose` | The state of the quote at the close
`MaxSpread`         | The widest relative spread of the candle
`OrderFlowImbalance` | The order flow imbalance of the quote updates (Cont, Kukanov and Stoikov)
`EffectiveSpread`   | The size-weighted effective spread of the trades against the mid price
`VolumeAtTouch`, `VolumeInsideSpread`, `VolumeThroughTouch` | The share of the volume traded at, inside or through the best bid and ask

And again, if these don't satisfy your needs, just bring your own by implementing the 
[CandleComponent](src/candle_components/candle_component_trait.rs) trait and you can plug them into your own candle struct.

## How to use:
To use this crate in your project, add the following to your Cargo.toml:

```toml
[dependencies]
trade_aggregation = "14"
```

Lets aggregate all trades into time based 1 minute candles, consisting of open, high, low and close information.
Notice how easy it is to specify the 'AggregationRule' and 'ModularCandle' being used in the process.
One can easily plug in their own implementation of those trait for full customization.
This examples uses an online style approach to update with each tick:

```rust
use trade_aggregation::{
    candle_components::{Close, High, Low, Open},
    *,
};

#[derive(Debug, Default, Clone, Candle)]
struct MyCandle {
    open: Open,
    high: High,
    low: Low,
    close: Close,
}

fn main() {
    let trades = load_trades_from_csv("data/Bitmex_XBTUSD_1M.csv")
        .expect("Could not load trades from file!");

    // specify the aggregation rule to be time based;
    // the unit of the timestamps comes from `Trade::TIMESTAMP_RESOLUTION`
    let time_rule = TimeRule::new(M1);
    // Notice how the aggregator is generic over the output candle type, 
    // the aggregation rule as well as the input trade data
    let mut aggregator = GenericAggregator::<MyCandle, TimeRule, Trade>::new(time_rule, false);

    // Receives the candles that each trade closes; reused to avoid allocations.
    let mut closed = Vec::new();
    for t in &trades {
        aggregator.update(t, &mut closed);
        for bar in closed.drain(..) {
            let candle = bar.candle();
            println!(
                "candle {} closed at {} with open: {}, high: {}, low: {}, close: {}",
                bar.seq(),
                bar.close_ts(),
                candle.open(),
                candle.high(),
                candle.low(),
                candle.close()
            );
        }
    }
}
```

Notice how the code is calling the 'open()', 'high()', 'low()' and 'close()' 
methods on the 'MyCandle' struct. 
These are getters automatically generated by the [Candle](trade_aggregation_derive/src/lib.rs) macro, 
that have the same name as the field.
In this case the 'Candle' derive macro for 'MyCandle' expands into about this,
for every event type `E` that all components accept:
```rust
use trade_aggregation::{
    candle_components::{Close, High, Low, Open},
    *,
};

#[derive(Debug, Default, Clone)]
struct MyCandle {
    open: Open,
    high: High,
    low: Low,
    close: Close,
}

impl MyCandle {
    const FIELDS: [&'static str; 4] = ["open", "high", "low", "close"];

    fn open(&self) -> f64 {
        self.open.value()
    }
    fn high(&self) -> f64 {
        self.high.value()
    }
    fn low(&self) -> f64 {
        self.low.value()
    }
    fn close(&self) -> f64 {
        self.close.value()
    }
}

impl<E> ModularCandle<E> for MyCandle
where
    Open: CandleComponentUpdate<E>,
    High: CandleComponentUpdate<E>,
    Low: CandleComponentUpdate<E>,
    Close: CandleComponentUpdate<E>,
{
    fn update(&mut self, event: &E) {
        self.open.update(event);
        self.high.update(event);
        self.low.update(event);
        self.close.update(event);
    }
    fn reset(&mut self) {
        self.open.reset();
        self.high.reset();
        self.low.reset();
        self.close.reset();
    }
    fn close_at(&mut self, timestamp: i64) {
        CandleComponentUpdate::<E>::close_at(&mut self.open, timestamp);
        CandleComponentUpdate::<E>::close_at(&mut self.high, timestamp);
        CandleComponentUpdate::<E>::close_at(&mut self.low, timestamp);
        CandleComponentUpdate::<E>::close_at(&mut self.close, timestamp);
    }
}
```

The generated code uses absolute paths, so the traits need not be in scope.
`#[candle(record = MyRecord, record_derive(Debug, Clone, Copy, serde::Serialize))]` also generates
a plain struct `MyRecord` with the value of each component and a method `record()`,
e.g. to store or send closed candles.

### Which candle gets the triggering trade
The second argument of `GenericAggregator::new` decides which candle receives the trade that triggers the rule.
`GenericAggregator::with_trigger_trade` takes a `TriggerTrade` instead:

'TriggerTrade'  | The triggering trade is ... | Every trade counted once | Close equals next open
----------------|-----------------------------|--------------------------|-----------------------
`NextCandle`    | the first trade of the next candle (`new(rule, false)`) | yes | no
`ClosingCandle` | the last trade of the closing candle | yes | no
`Both`          | in both candles (`new(rule, true)`) | no: `Volume`, `NumTrades` etc. count it twice | yes

This applies to the candles that the rule closes on an event.
A candle closed at a deadline never holds an event at or after the deadline.

See examples folder for more.
Run examples using
```ignore
cargo run --release --example aggregate_all_ohlc
cargo run --release --example streaming_aggregate_ohlc
```

## Performance:
To run the benchmarks, written using criterion, run:

```ignore
cargo bench
```

Here are some results running on a 12th gen Intel Core i7-12800H, aggregating 1 million trades into 1 minute candles:

Candle | Time
-------|-----------
Open   | 1     ms
OHLC   | 2.5   ms
All    | 8     ms

The more 'CandleComponent's you use, the longer it takes obviously.

### Features
The serde feature exists which, when enabled, derives Serialize and Deserialize
for the candle components, the rules and the `GenericAggregator` with its state.
A live process can save the aggregator with the offset of its input
and resume in the middle of a candle after a restart, with the same candles as without the restart.
Deserialize only state that this crate serialized: the rule parameters are checked when a rule is created, not when it is deserialized.

Save the state in a format that stores every `f64` exactly, e.g. [bincode](https://crates.io/crates/bincode).
Without its `float_roundtrip` feature, `serde_json` parses some numbers 1 ulp off,
and the restored aggregator then closes other candles than the saved one would have.


### TODOs:
- Make generic over the data type storing the price (`f64`, `f32`, `i64`, `Decimal`, etc...)
- Run many aggregators, rule parameters and symbols cheaply: see the proposal in [docs/2026-11-13_nxmxk_architecture.md](docs/2026-11-13_nxmxk_architecture.md)
- Rules and components on order book levels beyond the best bid and ask: see [docs/2026-11-13_order_book_beyond_l1.md](docs/2026-11-13_order_book_beyond_l1.md)

### Donations :moneybag: :money_with_wings:
I you would like to support the development of this crate, feel free to send over a donation:

Monero (XMR) address:
```plain
47xMvxNKsCKMt2owkDuN1Bci2KMiqGrAFCQFSLijWLs49ua67222Wu3LZryyopDVPYgYmAnYkSZSz9ZW2buaDwdyKTWGwwb
```

![monero](readme_img/monero_donations_qrcode.png)


### License
Copyright (C) 2020  <Mathis Wellmann wellmannmathis@gmail.com>

This program is free software: you can redistribute it and/or modify
it under the terms of the GNU Affero General Public License as published by
the Free Software Foundation, either version 3 of the License, or
(at your option) any later version.

This program is distributed in the hope that it will be useful,
but WITHOUT ANY WARRANTY; without even the implied warranty of
MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
GNU Affero General Public License for more details.

You should have received a copy of the GNU Affero General Public License
along with this program.  If not, see <https://www.gnu.org/licenses/>.

![GNU AGPLv3](readme_img/agplv3.png)

### Commercial License
If you'd like to use this crate legally without the restrictions of the GNU AGPLv3 license, 
please contact me so we can quickly arrange a custom license.
