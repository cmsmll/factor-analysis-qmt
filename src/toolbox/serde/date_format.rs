use serde::{Deserialize, Deserializer, Serializer, de::Error};
use time::{Date, format_description::well_known::Iso8601};

/// 将日期序列化为 `YYYY-MM-DD` 字符串。
pub fn serialize<S>(date: &Date, serializer: S) -> Result<S::Ok, S::Error>
where
    S: Serializer,
{
    serializer.serialize_str(&date.to_string())
}

/// 将 `YYYY-MM-DD` 字符串反序列化为日期。
pub fn deserialize<'de, D>(deserializer: D) -> Result<Date, D::Error>
where
    D: Deserializer<'de>,
{
    let value = String::deserialize(deserializer)?;
    Date::parse(&value, &Iso8601::DATE).map_err(D::Error::custom)
}

/// 将日期序列化为 `YYYY-MM-DD` 字符串数组。
pub fn serialize_datetime<S>(datetime: &[Date], serializer: S) -> Result<S::Ok, S::Error>
where
    S: Serializer,
{
    use serde::ser::SerializeSeq;

    let mut seq = serializer.serialize_seq(Some(datetime.len()))?;
    for date in datetime {
        seq.serialize_element(&date.to_string())?;
    }
    seq.end()
}

/// `Option<Date>` 的 `YYYY-MM-DD` 字符串序列化（`None` → `null`）。
pub mod opt {
    use serde::{Deserialize, Deserializer, Serializer, de::Error};
    use time::{Date, format_description::well_known::Iso8601};

    pub fn serialize<S>(date: &Option<Date>, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        match date {
            Some(date) => super::serialize(date, serializer),
            None => serializer.serialize_none(),
        }
    }

    pub fn deserialize<'de, D>(deserializer: D) -> Result<Option<Date>, D::Error>
    where
        D: Deserializer<'de>,
    {
        let value = Option::<String>::deserialize(deserializer)?;
        value
            .map(|v| Date::parse(&v, &Iso8601::DATE).map_err(D::Error::custom))
            .transpose()
    }
}
