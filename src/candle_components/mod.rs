// mod arithmetic_mean_price;
// mod close;
// mod directional_trade_ratio;
// mod directional_volume_ratio;
// mod high;
// mod low;
// mod num_trades;
mod open;
// mod std_dev_prices;
// mod std_dev_sizes;
// mod time_velocity;
// mod volume;
// mod weighted_price;

pub use open::Open;

#[cfg(test)]
mod tests {
    use crate::Trade;

    pub const TRADES: [Trade; 10] = [
        Trade {
            timestamp: 0,
            price: 100.0,
            size: 10.0,
        },
        Trade {
            timestamp: 1,
            price: 101.0,
            size: -10.0,
        },
        Trade {
            timestamp: 2,
            price: 100.0,
            size: 20.0,
        },
        Trade {
            timestamp: 3,
            price: 102.0,
            size: 10.0,
        },
        Trade {
            timestamp: 4,
            price: 103.0,
            size: 10.0,
        },
        Trade {
            timestamp: 5,
            price: 104.0,
            size: -20.0,
        },
        Trade {
            timestamp: 6,
            price: 102.0,
            size: -10.0,
        },
        Trade {
            timestamp: 7,
            price: 101.0,
            size: 10.0,
        },
        Trade {
            timestamp: 8,
            price: 102.0,
            size: 30.0,
        },
        Trade {
            timestamp: 9,
            price: 105.0,
            size: 10.0,
        },
    ];
}
