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

### TODOs:
- tests
- images to demonstrate the different methods
