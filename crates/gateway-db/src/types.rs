//! Cross-database compatible types for gateway-db.
//!
//! SQLite and PostgreSQL have different native type systems. This module
//! provides wrapper types that encode/decode correctly on both backends.

use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use std::fmt;
use std::ops::{Add, AddAssign, Deref, DerefMut, Div, DivAssign, Mul, MulAssign, Neg, Sub, SubAssign};
use std::str::FromStr;

/// Wrapper around [`Decimal`] that supports both PostgreSQL and SQLite.
///
/// - PostgreSQL: encoded/decoded as `NUMERIC`/`DECIMAL`
/// - SQLite: encoded/decoded as `TEXT`
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(transparent)]
pub struct DbDecimal(pub Decimal);

impl DbDecimal {
    pub const fn new(d: Decimal) -> Self {
        Self(d)
    }

    pub fn into_inner(self) -> Decimal {
        self.0
    }
}

impl Default for DbDecimal {
    fn default() -> Self {
        Self(Decimal::ZERO)
    }
}

impl Deref for DbDecimal {
    type Target = Decimal;
    fn deref(&self) -> &Decimal {
        &self.0
    }
}

impl DerefMut for DbDecimal {
    fn deref_mut(&mut self) -> &mut Decimal {
        &mut self.0
    }
}

impl From<Decimal> for DbDecimal {
    fn from(d: Decimal) -> Self {
        Self(d)
    }
}

impl From<DbDecimal> for Decimal {
    fn from(d: DbDecimal) -> Self {
        d.0
    }
}

impl fmt::Display for DbDecimal {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(f)
    }
}

// ── Arithmetic ───────────────────────────────────────────────────────────────

impl Add for DbDecimal {
    type Output = Self;
    fn add(self, rhs: Self) -> Self::Output {
        Self(self.0 + rhs.0)
    }
}

impl Add<Decimal> for DbDecimal {
    type Output = Self;
    fn add(self, rhs: Decimal) -> Self::Output {
        Self(self.0 + rhs)
    }
}

impl Add<DbDecimal> for Decimal {
    type Output = DbDecimal;
    fn add(self, rhs: DbDecimal) -> Self::Output {
        DbDecimal(self + rhs.0)
    }
}

impl Sub for DbDecimal {
    type Output = Self;
    fn sub(self, rhs: Self) -> Self::Output {
        Self(self.0 - rhs.0)
    }
}

impl Sub<Decimal> for DbDecimal {
    type Output = Self;
    fn sub(self, rhs: Decimal) -> Self::Output {
        Self(self.0 - rhs)
    }
}

impl Mul for DbDecimal {
    type Output = Self;
    fn mul(self, rhs: Self) -> Self::Output {
        Self(self.0 * rhs.0)
    }
}

impl Mul<Decimal> for DbDecimal {
    type Output = Self;
    fn mul(self, rhs: Decimal) -> Self::Output {
        Self(self.0 * rhs)
    }
}

impl Div for DbDecimal {
    type Output = Self;
    fn div(self, rhs: Self) -> Self::Output {
        Self(self.0 / rhs.0)
    }
}

impl Div<Decimal> for DbDecimal {
    type Output = Self;
    fn div(self, rhs: Decimal) -> Self::Output {
        Self(self.0 / rhs)
    }
}

impl AddAssign for DbDecimal {
    fn add_assign(&mut self, rhs: Self) {
        self.0 += rhs.0;
    }
}

impl AddAssign<Decimal> for DbDecimal {
    fn add_assign(&mut self, rhs: Decimal) {
        self.0 += rhs;
    }
}

impl SubAssign for DbDecimal {
    fn sub_assign(&mut self, rhs: Self) {
        self.0 -= rhs.0;
    }
}

impl MulAssign for DbDecimal {
    fn mul_assign(&mut self, rhs: Self) {
        self.0 *= rhs.0;
    }
}

impl DivAssign for DbDecimal {
    fn div_assign(&mut self, rhs: Self) {
        self.0 /= rhs.0;
    }
}

impl Neg for DbDecimal {
    type Output = Self;
    fn neg(self) -> Self::Output {
        Self(-self.0)
    }
}

impl PartialOrd for DbDecimal {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for DbDecimal {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.0.cmp(&other.0)
    }
}

impl std::iter::Sum for DbDecimal {
    fn sum<I: Iterator<Item = Self>>(iter: I) -> Self {
        Self(iter.map(|d| d.0).sum())
    }
}

impl<'a> std::iter::Sum<&'a DbDecimal> for DbDecimal {
    fn sum<I: Iterator<Item = &'a Self>>(iter: I) -> Self {
        Self(iter.map(|d| d.0).sum())
    }
}

impl std::iter::Sum<DbDecimal> for Decimal {
    fn sum<I: Iterator<Item = DbDecimal>>(iter: I) -> Self {
        iter.map(|d| d.0).sum()
    }
}

impl<'a> std::iter::Sum<&'a DbDecimal> for Decimal {
    fn sum<I: Iterator<Item = &'a DbDecimal>>(iter: I) -> Self {
        iter.map(|d| d.0).sum()
    }
}

impl AddAssign<DbDecimal> for Decimal {
    fn add_assign(&mut self, rhs: DbDecimal) {
        *self += rhs.0;
    }
}

impl TryFrom<DbDecimal> for f64 {
    type Error = rust_decimal::Error;
    fn try_from(d: DbDecimal) -> Result<Self, Self::Error> {
        d.0.try_into()
    }
}

impl TryFrom<DbDecimal> for i64 {
    type Error = rust_decimal::Error;
    fn try_from(d: DbDecimal) -> Result<Self, Self::Error> {
        d.0.try_into()
    }
}

// ── sqlx PostgreSQL ──────────────────────────────────────────────────────────

impl sqlx::Type<sqlx::Postgres> for DbDecimal {
    fn type_info() -> sqlx::postgres::PgTypeInfo {
        <Decimal as sqlx::Type<sqlx::Postgres>>::type_info()
    }
}

impl<'r> sqlx::Decode<'r, sqlx::Postgres> for DbDecimal {
    fn decode(value: sqlx::postgres::PgValueRef<'r>) -> Result<Self, Box<dyn std::error::Error + Send + Sync>> {
        <Decimal as sqlx::Decode<'r, sqlx::Postgres>>::decode(value).map(Self).map_err(|e| e as _)
    }
}

impl<'q> sqlx::Encode<'q, sqlx::Postgres> for DbDecimal {
    fn encode_by_ref(
        &self,
        buf: &mut sqlx::postgres::PgArgumentBuffer,
    ) -> Result<sqlx::encode::IsNull, Box<dyn std::error::Error + Send + Sync>> {
        <Decimal as sqlx::Encode<'q, sqlx::Postgres>>::encode_by_ref(&self.0, buf)
            .map_err(|e| e as _)
    }
}

// ── sqlx SQLite ──────────────────────────────────────────────────────────────

impl sqlx::Type<sqlx::Sqlite> for DbDecimal {
    fn type_info() -> sqlx::sqlite::SqliteTypeInfo {
        <String as sqlx::Type<sqlx::Sqlite>>::type_info()
    }
}

impl<'r> sqlx::Decode<'r, sqlx::Sqlite> for DbDecimal {
    fn decode(value: sqlx::sqlite::SqliteValueRef<'r>) -> Result<Self, Box<dyn std::error::Error + Send + Sync>> {
        let s: &str = <&str as sqlx::Decode<'r, sqlx::Sqlite>>::decode(value)?;
        Decimal::from_str(s)
            .map(Self)
            .map_err(|e| Box::new(e) as Box<dyn std::error::Error + Send + Sync>)
    }
}

impl<'q> sqlx::Encode<'q, sqlx::Sqlite> for DbDecimal {
    fn encode_by_ref(
        &self,
        buf: &mut Vec<sqlx::sqlite::SqliteArgumentValue<'q>>,
    ) -> Result<sqlx::encode::IsNull, Box<dyn std::error::Error + Send + Sync>> {
        let s = self.0.to_string();
        <String as sqlx::Encode<'q, sqlx::Sqlite>>::encode_by_ref(&s, buf)
            .map_err(|e| e as _)
    }
}

// ── JsonVec<T> ───────────────────────────────────────────────────────────────

/// Wrapper around `Vec<T>` that stores as JSON TEXT in SQLite
/// and as a native array in PostgreSQL.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(transparent)]
pub struct JsonVec<T>(pub Vec<T>);

impl<T> Default for JsonVec<T> {
    fn default() -> Self {
        Self(Vec::new())
    }
}

impl<T> Deref for JsonVec<T> {
    type Target = Vec<T>;
    fn deref(&self) -> &Vec<T> {
        &self.0
    }
}

impl<T> DerefMut for JsonVec<T> {
    fn deref_mut(&mut self) -> &mut Vec<T> {
        &mut self.0
    }
}

impl<T> From<Vec<T>> for JsonVec<T> {
    fn from(v: Vec<T>) -> Self {
        Self(v)
    }
}

impl<T> From<JsonVec<T>> for Vec<T> {
    fn from(v: JsonVec<T>) -> Self {
        v.0
    }
}

impl<T> IntoIterator for JsonVec<T> {
    type Item = T;
    type IntoIter = std::vec::IntoIter<T>;
    fn into_iter(self) -> Self::IntoIter {
        self.0.into_iter()
    }
}

impl<'a, T> IntoIterator for &'a JsonVec<T> {
    type Item = &'a T;
    type IntoIter = std::slice::Iter<'a, T>;
    fn into_iter(self) -> Self::IntoIter {
        self.0.iter()
    }
}

// ── sqlx PostgreSQL ──────────────────────────────────────────────────────────

impl<T: sqlx::Type<sqlx::Postgres> + sqlx::postgres::PgHasArrayType + Send> sqlx::Type<sqlx::Postgres>
    for JsonVec<T>
{
    fn type_info() -> sqlx::postgres::PgTypeInfo {
        <Vec<T> as sqlx::Type<sqlx::Postgres>>::type_info()
    }
}

impl<'r, T: for<'a> sqlx::Decode<'a, sqlx::Postgres> + sqlx::Type<sqlx::Postgres> + sqlx::postgres::PgHasArrayType>
    sqlx::Decode<'r, sqlx::Postgres> for JsonVec<T>
{
    fn decode(value: sqlx::postgres::PgValueRef<'r>) -> Result<Self, Box<dyn std::error::Error + Send + Sync>> {
        <Vec<T> as sqlx::Decode<'r, sqlx::Postgres>>::decode(value)
            .map(JsonVec)
            .map_err(|e| e as _)
    }
}

impl<'q, T: for<'a> sqlx::Encode<'a, sqlx::Postgres> + sqlx::Type<sqlx::Postgres> + sqlx::postgres::PgHasArrayType + Send>
    sqlx::Encode<'q, sqlx::Postgres> for JsonVec<T>
{
    fn encode_by_ref(
        &self,
        buf: &mut sqlx::postgres::PgArgumentBuffer,
    ) -> Result<sqlx::encode::IsNull, Box<dyn std::error::Error + Send + Sync>> {
        <Vec<T> as sqlx::Encode<'q, sqlx::Postgres>>::encode_by_ref(&self.0, buf)
            .map_err(|e| e as _)
    }
}

// ── sqlx SQLite ──────────────────────────────────────────────────────────────

impl<T: Send> sqlx::Type<sqlx::Sqlite> for JsonVec<T> {
    fn type_info() -> sqlx::sqlite::SqliteTypeInfo {
        <String as sqlx::Type<sqlx::Sqlite>>::type_info()
    }
}

impl<'r, T: serde::Deserialize<'r>> sqlx::Decode<'r, sqlx::Sqlite> for JsonVec<T> {
    fn decode(value: sqlx::sqlite::SqliteValueRef<'r>) -> Result<Self, Box<dyn std::error::Error + Send + Sync>> {
        let s: &str = <&str as sqlx::Decode<'r, sqlx::Sqlite>>::decode(value)?;
        serde_json::from_str(s)
            .map(JsonVec)
            .map_err(|e| Box::new(e) as _)
    }
}

impl<'q, T: serde::Serialize + Send> sqlx::Encode<'q, sqlx::Sqlite> for JsonVec<T> {
    fn encode_by_ref(
        &self,
        buf: &mut Vec<sqlx::sqlite::SqliteArgumentValue<'q>>,
    ) -> Result<sqlx::encode::IsNull, Box<dyn std::error::Error + Send + Sync>> {
        let s = serde_json::to_string(&self.0)?;
        <String as sqlx::Encode<'q, sqlx::Sqlite>>::encode_by_ref(&s, buf)
    }
}
