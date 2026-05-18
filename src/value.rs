use std::borrow::Cow;
use std::cmp::Ordering;

#[derive(Debug, Clone)]
pub enum Value<'a> {
    Null,
    Bool(bool),
    Int(i64),
    UInt(u64),
    Float(f64),
    String(Cow<'a, str>),
    Seq(Vec<Value<'a>>),
    Map(Vec<(Value<'a>, Value<'a>)>),
    Tagged(Cow<'a, str>, Box<Value<'a>>),
}

/// Discriminant ordering for cross-variant comparisons.
/// Chosen so that "simpler" variants sort before more-complex ones.
fn discriminant_rank(v: &Value<'_>) -> u8 {
    match v {
        Value::Null => 0,
        Value::Bool(_) => 1,
        Value::Int(_) => 2,
        Value::UInt(_) => 3,
        Value::Float(_) => 4,
        Value::String(_) => 5,
        Value::Seq(_) => 6,
        Value::Map(_) => 7,
        Value::Tagged(_, _) => 8,
    }
}

impl PartialEq for Value<'_> {
    fn eq(&self, other: &Self) -> bool {
        self.cmp(other) == Ordering::Equal
    }
}

impl Eq for Value<'_> {}

impl Ord for Value<'_> {
    fn cmp(&self, other: &Self) -> Ordering {
        let ra = discriminant_rank(self);
        let rb = discriminant_rank(other);
        if ra != rb {
            return ra.cmp(&rb);
        }
        match (self, other) {
            (Value::Null, Value::Null) => Ordering::Equal,
            (Value::Bool(a), Value::Bool(b)) => a.cmp(b),
            (Value::Int(a), Value::Int(b)) => a.cmp(b),
            (Value::UInt(a), Value::UInt(b)) => a.cmp(b),
            // total_cmp gives a total order including NaN (NaN compares Equal to NaN)
            (Value::Float(a), Value::Float(b)) => a.total_cmp(b),
            (Value::String(a), Value::String(b)) => a.cmp(b),
            (Value::Seq(a), Value::Seq(b)) => a.cmp(b),
            (Value::Map(a), Value::Map(b)) => a.cmp(b),
            (Value::Tagged(ta, ia), Value::Tagged(tb, ib)) => {
                ta.cmp(tb).then_with(|| ia.cmp(ib))
            }
            _ => unreachable!("same rank implies same variant"),
        }
    }
}

impl PartialOrd for Value<'_> {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

