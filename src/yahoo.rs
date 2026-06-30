use chrono::{DateTime, NaiveDate};
use serde_json::{Map, Value};

use std::collections::BTreeMap;
use std::error::Error;

#[derive(Debug)]
pub struct Chart {
    pub symbol: String,
    pub quotes: BTreeMap<NaiveDate, Quote>,
    pub dividends: BTreeMap<NaiveDate, f64>,
    pub splits: BTreeMap<NaiveDate, Split>,
}

#[derive(Debug)]
pub struct Quote {
    pub open: f64,
    pub high: f64,
    pub low: f64,
    pub close: f64,
}

#[derive(Debug)]
pub struct Split {
    pub numerator: f64,
    pub denominator: f64,
}

pub fn parse_chart(s: &String) -> Result<Chart, Box<dyn Error + Send + Sync>> {
    let input: Map<String, Value> = serde_json::from_str(&s)?;

    let chart: &Map<String, Value> = {
        let value = input.get("chart").ok_or("missing 'chart'")?;
        value.as_object().ok_or("not object")?
    };
    if !matches!(chart.get("error"), Some(Value::Null)) {
        return Err("non-null error".into());
    }

    let result: &Map<String, Value> = {
        let value = chart.get("result").ok_or("missing 'result'")?;
        let array = value.as_array().ok_or("not array")?;
        if array.len() != 1 {
            return Err("array length != 1".into());
        }
        array[0].as_object().ok_or("not object")?
    };

    let symbol: String = {
        let meta: &Map<String, Value> = {
            let value = result.get("meta").ok_or("missing 'meta'")?;
            value.as_object().ok_or("not object")?
        };
        let value = meta.get("symbol").ok_or("missing 'symbol'")?;
        value.as_str().ok_or("not string")?.to_owned()
    };

    let dates: Vec<NaiveDate> = {
        let value = result.get("timestamp").ok_or("missing 'timestamp'")?;
        let timestamps: &Vec<Value> = value.as_array().ok_or("not_array")?;
        timestamps
            .into_iter()
            .map(|ts| -> Result<NaiveDate, Box<dyn Error + Send + Sync>> {
                let ts = ts
                    .as_number()
                    .ok_or("not number")?
                    .as_i64()
                    .ok_or("not i64")?;
                Ok(DateTime::from_timestamp(ts, 0)
                    .map(|dt| dt.naive_utc().date())
                    .ok_or("invalid ts")?)
            })
            .collect::<Result<Vec<NaiveDate>, Box<dyn Error + Send + Sync>>>()?
    };
    if dates.is_empty() {
        return Err("no dates".into());
    }

    let quotes: BTreeMap<NaiveDate, Quote> = {
        let mut quotes: BTreeMap<NaiveDate, Quote> = BTreeMap::new();

        let quote: &Map<String, Value> = {
            let indicators: &Map<String, Value> = {
                let value = result.get("indicators").ok_or("missing 'indicators'")?;
                value.as_object().ok_or("not object")?
            };

            let value = indicators.get("quote").ok_or("missing 'quote'")?;
            let array = value.as_array().ok_or("not array")?;
            if array.len() != 1 {
                return Err("array length != 1".into());
            }
            array[0].as_object().ok_or("not object")?
        };

        let subquotes = |name: &str| -> Result<Vec<f64>, Box<dyn Error + Send + Sync>> {
            let value = quote.get(name).ok_or(format!("missing '{}'", name))?;
            let array: &Vec<Value> = value.as_array().ok_or("not array")?;

            let retval: Vec<f64> = array
                .into_iter()
                .map(|e| -> Result<f64, Box<dyn Error + Send + Sync>> {
                    Ok(e.as_number()
                        .ok_or("not number")?
                        .as_f64()
                        .ok_or("not f64")?)
                })
                .collect::<Result<Vec<f64>, Box<dyn Error + Send + Sync>>>()?;

            if retval.len() != dates.len() {
                return Err("length != dates.len()".into());
            }

            Ok(retval)
        };

        let opens: Vec<f64> = subquotes("open")?;
        let highs: Vec<f64> = subquotes("high")?;
        let lows: Vec<f64> = subquotes("low")?;
        let closes: Vec<f64> = subquotes("close")?;

        for i in 0..dates.len() {
            let (open, high, low, close) = (opens[i], highs[i], lows[i], closes[i]);

            if open > high || open < low {
                return Err(format!("{}: invalid open {} {} {}", symbol, open, high, low).into());
            }
            if close > high || close < low {
                return Err(format!("{}: invalid close {} {} {}", symbol, close, high, low).into());
            }

            quotes.insert(
                dates[i],
                Quote {
                    open,
                    high,
                    low,
                    close,
                },
            );
        }

        quotes
    };

    let events: &Map<String, Value> = {
        match result.get("events") {
            Some(e) => e.as_object().ok_or("not object")?,
            None => &Map::new(),
        }
    };

    let dividends: BTreeMap<NaiveDate, f64> = {
        let mut dividends: BTreeMap<NaiveDate, f64> = BTreeMap::new();

        let object: &Map<String, Value> = match events.get("dividends") {
            Some(d) => d.as_object().ok_or("not object")?,
            None => &Map::new(),
        };
        for (key, value) in object {
            let date = DateTime::from_timestamp(key.parse::<i64>()?, 0)
                .map(|dt| dt.naive_utc().date())
                .ok_or("invalid ts")?;
            let amount = value
                .as_object()
                .ok_or("not object")?
                .get("amount")
                .ok_or("missing 'amount'")?
                .as_number()
                .ok_or("not number")?
                .as_f64()
                .ok_or("not f64")?;
            dividends.insert(date, amount);
        }

        dividends
    };

    let splits: BTreeMap<NaiveDate, Split> = {
        let mut splits: BTreeMap<NaiveDate, Split> = BTreeMap::new();

        let object: &Map<String, Value> = match events.get("splits") {
            Some(s) => s.as_object().ok_or("not object")?,
            None => &Map::new(),
        };
        for (key, value) in object {
            let date = DateTime::from_timestamp(key.parse::<i64>()?, 0)
                .map(|dt| dt.naive_utc().date())
                .ok_or("invalid ts")?;

            let split: &Map<String, Value> = value.as_object().ok_or("not object")?;
            let numerator: f64 = split
                .get("numerator")
                .ok_or("missing 'numerator'")?
                .as_number()
                .ok_or("not number")?
                .as_f64()
                .ok_or("not f64")?;
            let denominator: f64 = split
                .get("denominator")
                .ok_or("missing 'denominator'")?
                .as_number()
                .ok_or("not number")?
                .as_f64()
                .ok_or("not f64")?;

            splits.insert(
                date,
                Split {
                    numerator,
                    denominator,
                },
            );
        }

        splits
    };

    Ok(Chart {
        symbol,
        quotes,
        dividends,
        splits,
    })
}
