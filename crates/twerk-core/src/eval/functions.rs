//! Built-in evaluation functions.
//!
//! Provides pure calculation functions for evalexpr:
//! - `randomInt()` - random integer generation
//! - `sequence()` - integer range generation
//! - `split()` - string splitting

use evalexpr::Value;

/// Generates a random integer, optionally bounded by a maximum value.
///
/// # Arguments
/// * `args` - Either Empty for unbounded, or an Int for the exclusive upper bound
///
/// # Returns
/// A random integer in the range [0, max) if max is provided and positive,
/// or a random positive integer if no argument is provided.
///
/// # Errors
/// Returns an error if the argument is not a valid numeric type.
pub fn random_int_fn(args: &Value) -> Result<Value, String> {
    let max_opt = match args.as_tuple() {
        Ok(tuple) => match tuple.len() {
            0 => None,
            1 => Some(
                tuple[0]
                    .as_int()
                    .map_err(|_| "randomInt requires a numeric argument")?,
            ),
            n => return Err(format!("randomInt expects 0 or 1 arguments, got {n}")),
        },
        Err(_) => match args {
            Value::Empty => None,
            Value::Int(n) => Some(*n),
            _ => return Err("randomInt requires a numeric argument".into()),
        },
    };

    match max_opt {
        None | Some(0) => {
            let val = rand::random::<i64>();
            Ok(Value::Int(
                i64::try_from(val.unsigned_abs()).map_or(i64::MAX, |v| v),
            ))
        }
        Some(max) if max > 0 => {
            let val = rand::random::<i64>();
            let val_abs = val.unsigned_abs();
            let result = val_abs % u64::try_from(max).map_or(u64::MAX, |v| v);
            Ok(Value::Int(i64::try_from(result).map_or(i64::MAX, |v| v)))
        }
        Some(_) => Ok(Value::Int(0)),
    }
}

/// Generates a sequence of integers from start (inclusive) to stop (exclusive).
///
/// # Arguments
/// * `args` - A tuple of (start, stop) integers
///
/// # Returns
/// A tuple of integers in the range [start, stop), or an empty tuple if start >= stop.
///
/// # Errors
/// Returns an error if the arguments are not numeric or not exactly 2 arguments.
pub fn sequence_fn(args: &Value) -> Result<Value, String> {
    let tuple = args
        .as_tuple()
        .map_err(|_| "sequence expects tuple arguments".to_string())?;

    if tuple.len() != 2 {
        return Err(format!("sequence expects 2 arguments, got {}", tuple.len()));
    }

    let start = tuple[0]
        .as_int()
        .map_err(|_| "sequence requires numeric arguments".to_string())?;
    let stop = tuple[1]
        .as_int()
        .map_err(|_| "sequence requires numeric arguments".to_string())?;

    let range = if start >= stop {
        Vec::new()
    } else {
        (start..stop).map(Value::Int).collect()
    };

    Ok(Value::Tuple(range))
}

/// Splits a string by a delimiter into a tuple of strings.
///
/// # Arguments
/// * `args` - A tuple of (string, delimiter)
///
/// # Returns
/// A tuple of string segments.
///
/// # Errors
/// Returns an error if the arguments are not strings or not exactly 2 arguments.
pub fn split_fn(args: &Value) -> Result<Value, String> {
    let tuple = args
        .as_tuple()
        .map_err(|_| "split expects tuple arguments".to_string())?;

    if tuple.len() != 2 {
        return Err(format!("split expects 2 arguments, got {}", tuple.len()));
    }

    let s = tuple[0]
        .as_string()
        .map_err(|_| "split requires string arguments".to_string())?;
    let delimiter = tuple[1]
        .as_string()
        .map_err(|_| "split requires string arguments".to_string())?;

    let parts: Vec<Value> = s
        .split(delimiter.as_str())
        .map(|p| Value::String(p.to_string()))
        .collect();
    Ok(Value::Tuple(parts))
}
