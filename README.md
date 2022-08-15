# Trade Aggregation
A high performance, modular and flexible trade aggregation crate, producing Candle data, suitable for low-latency applications.
It allows the user to choose the rule dictating how a new candle is created 
through the [AggregationRule](src/aggregation_rules/aggregation_rule_trait.rs) trait, 
e.g: Time, Volume based or some other information driven rule.
It also allows the user to choose which type of candle will be created from the aggregation process
through the [ModularCandle](src/modular_candle_trait.rs) trait. Combined with the [Candle](trade_aggregation_derive/src/lib.rs) macro, 
it enables the user to flexibly create any type of Candle as long as each component implements 
the [CandleComponent](src/candle_components/candle_component_trait.rs) trait.

See [MathisWellmann/go_trade_aggregation](https://github.com/MathisWellmann/go_trade_aggregation) for a go implementation with less features and performance.

### How to use:
To use this crate in your project, add the following to your Cargo.toml:

```toml
[dependencies]
trade_aggregation = "^3"
```

Aggregate all trades by volume at once:

```rust
extern crate trade_aggregation;
use trade_aggregation::{aggregate_all_trades, load_trades_from_csv, By, VolumeAggregator};

fn main() {
    // load trades from file
    let trades = load_trades_from_csv("data/Bitmex_XBTUSD_1M.csv").unwrap();
    // create the desired volume aggregator with parameters
    let mut aggregator = VolumeAggregator::new(1000.0, By::Base);

    let candles = aggregate_all_trades(&trades, &mut aggregator);

    for c in &candles {
        println!("candle: {:?}", c);
    }
}
```

Or Use streaming trades to update with each tick:

```rust
extern crate trade_aggregation;
use trade_aggregation::{load_trades_from_csv, Aggregator, VolumeAggregator, By};

fn main() {
    // load trades from file
    let trades = load_trades_from_csv("data/Bitmex_XBTUSD_1M.csv").unwrap();

    // create new streaming aggregator based on volume
    let mut agg_volume = VolumeAggregator::new(1000.0, By::Base);

    for t in &trades {
        // update using the latest trade
        match agg_volume.update(t) {
            Some(candle) => {
                // do something with the latest candle
                println!("candle: {:?}", candle);
            },
            None => {}
        }
    }
}
```

See examples folder for more.
Run examples using
```
cargo run --example simple_volume
cargo run --example simple_time
cargo run --example streaming_time
cargo run --example streaming_volume
```

### Performance:
To run the benchmarks, written using criterion, run:

```shell
cargo bench
```

Here are some results running on a 12th gen Intel Core i7-12800H, aggregating 1 million trades into 1 minute candles:

Candle | Time in ms
-------|-----------
Open   | 1.8
OHLC   | 7
All    | 16ms

The more 'CandleComponent's you use, the longer it takes obviously.

### Donations :moneybag: :money_with_wings:
I you would like to support the development of this crate, feel free to send over a donation:

Monero (XMR) address:
```plain
47xMvxNKsCKMt2owkDuN1Bci2KMiqGrAFCQFSLijWLs49ua67222Wu3LZryyopDVPYgYmAnYkSZSz9ZW2buaDwdyKTWGwwb
```

![monero](img/monero_donations_qrcode.png)


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

![GNU AGPLv3](img/agplv3.png)

### Commercial License
If you'd like to use this crate legally without the restrictions of the GNU AGPLv3 license, 
please contact me so we can quickly arrange a custom license.
