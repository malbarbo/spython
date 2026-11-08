//! The name that CPython suggests for a `NameError`, from
//! `Python/suggestions.c` of CPython 3.13.

/// A list of this many candidates or more gives no suggestion.
pub const MAX_CANDIDATE_ITEMS: usize = 750;
const MAX_STRING_SIZE: usize = 40;
const MOVE_COST: usize = 2;
const CASE_COST: usize = 1;

/// The most that `candidate` may be away from `name` and still be a
/// suggestion. No more than about a third of the bytes may need a change.
pub fn max_distance(name: &str, candidate: &str) -> usize {
    (name.len() + candidate.len() + 3) * MOVE_COST / 6
}

/// The distance from `a` to `b` over their UTF-8 bytes, and a value above
/// `max_cost` when the distance is above `max_cost`, or when one of them is
/// longer than 40 bytes after their common prefix and suffix.
pub fn distance(a: &str, b: &str, max_cost: usize) -> usize {
    let (mut a, mut b) = (a.as_bytes(), b.as_bytes());
    while let (Some((x, a_rest)), Some((y, b_rest))) = (a.split_first(), b.split_first())
        && x == y
    {
        (a, b) = (a_rest, b_rest);
    }
    while let (Some((x, a_rest)), Some((y, b_rest))) = (a.split_last(), b.split_last())
        && x == y
    {
        (a, b) = (a_rest, b_rest);
    }
    if a.is_empty() || b.is_empty() {
        return (a.len() + b.len()) * MOVE_COST;
    }
    if a.len() > MAX_STRING_SIZE || b.len() > MAX_STRING_SIZE {
        return max_cost + 1;
    }
    if b.len() < a.len() {
        std::mem::swap(&mut a, &mut b);
    }
    if (b.len() - a.len()) * MOVE_COST > max_cost {
        return max_cost + 1;
    }
    // One row of the matrix, updated in place.
    let mut row: Vec<usize> = (1..=a.len()).map(|i| i * MOVE_COST).collect();
    let mut result = 0;
    for (b_index, &code) in b.iter().enumerate() {
        let mut distance = b_index * MOVE_COST;
        result = distance;
        let mut minimum = usize::MAX;
        for (cell, &a_code) in row.iter_mut().zip(a) {
            let substitute = distance + substitution_cost(code, a_code);
            distance = *cell;
            let insert_delete = result.min(distance) + MOVE_COST;
            result = insert_delete.min(substitute);
            *cell = result;
            minimum = minimum.min(result);
        }
        if minimum > max_cost {
            return max_cost + 1;
        }
    }
    result
}

fn substitution_cost(a: u8, b: u8) -> usize {
    if a & 31 != b & 31 {
        return MOVE_COST;
    }
    if a == b {
        return 0;
    }
    if a.eq_ignore_ascii_case(&b) {
        CASE_COST
    } else {
        MOVE_COST
    }
}
