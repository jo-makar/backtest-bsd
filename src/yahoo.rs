use chrono::{DateTime, NaiveDate};
use serde_json::{Map, Value};

use std::error::Error;

#[derive(Debug)]
pub struct Chart {
    pub symbol: String,
}

pub fn parse_chart(s: &String) -> Result<Chart, Box<dyn Error + Send + Sync>> {
    let input: Map<String, Value> = serde_json::from_str(&s)?;

    let chart: &Map<String, Value> = {
        let value = input.get("chart").ok_or("missing 'chart'")?;
        value.as_object().ok_or("not object")?
    };
    if !matches!(chart.get("error"), Some(Value::Null)) {
        return Err(format!("non-null error").into());
    }

    let result: &Map<String, Value> = {
        let value = chart.get("result").ok_or("missing 'result'")?;
        let array = value.as_array().ok_or("not array")?;
        if array.len() != 1 {
            return Err(format!("array length != 1").into());
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
        let value = result
            .get("timestamp")
            .ok_or(format!("{} missing 'timestamp'", symbol))?;

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
        return Err(format!("no dates").into());
    }

    // FIXME STOPPED

    Ok(Chart { symbol })
}
