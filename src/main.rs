use trade_aggregation::Trade;
use trade_aggregation::{
    candle_components::{CandleComponent, Close, Open},
    load_trades_from_csv, ModularCandle, TimeRule, M1,
};
use trade_aggregation_derive::Candle;

#[derive(Default, Debug, Clone, Candle)]
struct MyCandle {
    open: Open,
    close: Close,
}

fn main() {
    let my_candle = MyCandle::default();
}
