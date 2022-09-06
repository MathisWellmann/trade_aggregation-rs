use crate::{
    candle_components::{CandleComponent, Close, High, Low, Open},
    ModularCandle, Trade,
};
use plotters::prelude::*;
use trade_aggregation_derive::Candle;

#[derive(Debug, Default, Clone, Candle)]
pub(crate) struct OhlcCandle {
    open: Open,
    high: High,
    low: Low,
    close: Close,
}

/// Creates a plot of OHLC candles
pub(crate) fn plot_ohlc_candles(
    candles: &[OhlcCandle],
    filename: &str,
    dims: (u32, u32),
) -> Result<(), Box<dyn std::error::Error>> {
    let n = candles.len();

    let mut candlesticks: Vec<CandleStick<f64, f64>> = Vec::with_capacity(n);
    let mut price_min = f64::MAX;
    let mut price_max = f64::MIN;
    let candle_width = (dims.0 / n as u32) - 1;
    for (i, c) in candles.iter().enumerate() {
        candlesticks.push(CandleStick::new(
            i as f64,
            c.open(),
            c.high(),
            c.low(),
            c.close(),
            &GREEN,
            &RED,
            candle_width,
        ));

        if c.low() < price_min {
            price_min = c.low();
        }
        if c.high() > price_max {
            price_max = c.high();
        }
    }

    let root_area = BitMapBackend::new(filename, dims).into_drawing_area();

    root_area.fill(&WHITE)?;
    let root_area = root_area.titled(filename, ("sans-serif", 20).into_font())?;

    let mut cc0 = ChartBuilder::on(&root_area)
        .margin(20)
        .set_all_label_area_size(20)
        .caption("Candles", ("sans-serif", 20).into_font())
        .build_cartesian_2d(0_f64..candles.len() as f64, price_min..price_max)?;

    cc0.configure_mesh().x_labels(20).y_labels(20).draw()?;

    cc0.draw_series(candlesticks)?;

    Ok(())
}
