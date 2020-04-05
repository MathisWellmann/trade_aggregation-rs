# Trade Aggregation
Convert trade data into candles using different forms of aggregation.
The Candles provide more detailed statistics than the usual OHLCV candles.
Additional statistics inlcude:
- number of trades
- trade direction ratio ( num_buys / num_trades )
- volume direction ratio ( buyVolume / totalVolume )
- weighted average price ( using abs(size) as weight)

This Aggregation package allows for the creation of highly sophisticated algorithm and ML models. By providing a streaming interface, one is able to build real-time trading agents.
It enables a clear view of the market state without using arbitrary time aggregation.
See [MathisWellmann/go_trade_aggregation](https://github.com/MathisWellmann/go_trade_aggregation) for a go implementation.

### How to use:
aggregate all trades by volume at once
```
extern crate trade_aggregation;
use trade_aggregation::{common, agg_volume};

fn main() {
    // load trades from file
    let trades = common::load_trades_from_csv("data/Bitmex_XBTUSD_1M.csv");

    let threshold = 1000.0;  // ~volume in each candle
    let by = common::BASE;  // take USD as volume measure
    // let by = common::ASSET;  // take BTC as volume measure
    let candles = agg_volume::agg_volume(&trades, threshold, by);

    for i in 0..candles.len() {
        println!("candle: {:?}", candles[i]);
    }
}
```
Use streaming trades to update with each tick
```
extern crate trade_aggregation;
use trade_aggregation::{common, agg_volume_streaming};

fn main() {
    // load trades from file
    let trades = common::load_trades_from_csv("data/Bitmex_XBTUSD_1M.csv");

    // create new streaming aggregator based on volume
    let threshold = 1000.0;  // threshold of volume in this case denoted in BASE currency which is USD
    let by = common::BASE;  // take USD as volume measure
    // let by = common::ASSET;  // take BTC as volume measure

    for i in 0..trades.len() {
        // update using the latest trade
        let new_candle = agg_volume.update(&trades[i]);
        // if new candle has been created
        if new_candle {
            // access latest candle
            let candle = agg_volume.last();
            println!("candle: {:?}", candle);
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

### Time Aggregation:
Creates a candle every n seconds.
This method of aggregating trades has a long history mostly due to humans interacting with the market with a perception of time.
This is however not how the market works (especially 24/7 crypto markets).
The markets doesnt care about the time, only about price and volume.

### Volume Aggregation:
Creates a candle every n traded contracts ( trade size)
Price moves occur whenever and aggressive trader places a market order with a given size.
The more size is being traded the more likely a stronger move will be.
This is more natural way to view the market and provides many advantages over time aggregation such as more well behaved volatility.
In this mode of Aggregation, candles will be printed dynamically and will print more candles in times of higher volume / volatility,
therefore providing the trader which an automatically scaling view, just like as if he would switch time periods, but way better.
