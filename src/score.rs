use serde::{Deserialize, Deserializer, Serialize, Serializer};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord)]
pub struct Score(i64);

impl Score {
    pub fn raw(self) -> i64 {
        self.0
    }

    pub fn from_raw(v: i64) -> Self {
        Score(v)
    }
}

impl Serialize for Score {
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        s.serialize_f64(self.0 as f64 / 100_000.0)
    }
}

impl<'de> Deserialize<'de> for Score {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        let f = f64::deserialize(d)?;
        Ok(Score((f * 100_000.0).round() as i64))
    }
}

impl sqlx::Type<sqlx::Sqlite> for Score {
    fn type_info() -> sqlx::sqlite::SqliteTypeInfo {
        <i64 as sqlx::Type<sqlx::Sqlite>>::type_info()
    }
}

impl<'r> sqlx::Decode<'r, sqlx::Sqlite> for Score {
    fn decode(
        value: <sqlx::Sqlite as sqlx::Database>::ValueRef<'r>,
    ) -> Result<Self, sqlx::error::BoxDynError> {
        Ok(Score(<i64 as sqlx::Decode<'r, sqlx::Sqlite>>::decode(value)?))
    }
}

impl<'q> sqlx::Encode<'q, sqlx::Sqlite> for Score {
    fn encode_by_ref(
        &self,
        buf: &mut <sqlx::Sqlite as sqlx::Database>::ArgumentBuffer<'q>,
    ) -> Result<sqlx::encode::IsNull, sqlx::error::BoxDynError> {
        <i64 as sqlx::Encode<'q, sqlx::Sqlite>>::encode_by_ref(&self.0, buf)
    }
}

impl std::ops::Add for Score {
    type Output = Score;
    fn add(self, rhs: Score) -> Score {
        Score(self.0.checked_add(rhs.0).expect("Score overflow in Add"))
    }
}

impl std::iter::Sum for Score {
    fn sum<I: Iterator<Item = Self>>(iter: I) -> Self {
        iter.fold(Score(0), |acc, s| {
            Score(acc.0.checked_add(s.0).expect("Score overflow in Sum"))
        })
    }
}
