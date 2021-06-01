# Trade Aggregation
Convert trade data into candles.

See [MathisWellmann/go_trade_aggregation](https://github.com/MathisWellmann/go_trade_aggregation) for a go implementation with less features though.

### How to use:
To use this crate in your project, add the following to your Cargo.toml:

```toml
[dependencies]
trade_aggregation = "1.0.1"
```

Aggregate all trades by volume at once:

```rust
extern crate trade_aggregation;
use trade_aggregation::{aggregate_all_trades, load_trades_from_csv, By, VolumeAggregator};

fn main() {
    // load trades from file
    let trades = load_trades_from_csv("data/Bitmex_XBTUSD_1M.csv");
    // create the desired volume aggregator with parameters
    let mut aggregator = VolumeAggregator::new(1000.0, By::Base);

    let candles = aggregate_all_trades(&trades, &mut aggregator);

    for c in &candles {
        println!("candle: {:?}", c);
    }
}
```

Use streaming trades to update with each tick:

```rust
extern crate trade_aggregation;
use trade_aggregation::{load_trades_from_csv, Aggregator, VolumeAggregator, By};

fn main() {
    // load trades from file
    let trades = load_trades_from_csv("data/Bitmex_XBTUSD_1M.csv");

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
