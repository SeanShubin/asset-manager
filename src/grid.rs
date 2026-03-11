//! Grid size calculation helpers (shared between viewport and detail panel).

/// All divisors of `dim` that are >= 8.
pub fn valid_cell_sizes(dim: u32) -> Vec<u32> {
    (8..=dim).filter(|&s| dim % s == 0).collect()
}

pub fn prev_valid_size(valid: &[u32], current: u32, dim: u32) -> u32 {
    if valid.is_empty() {
        return (8..current).rev().find(|&d| dim % d == 0).unwrap_or(current);
    }
    valid.iter().copied().rev().find(|&s| s < current).unwrap_or(current)
}

pub fn next_valid_size(valid: &[u32], current: u32, dim: u32) -> u32 {
    if valid.is_empty() {
        return ((current + 1)..=dim).find(|&d| d >= 8 && dim % d == 0).unwrap_or(current);
    }
    valid.iter().copied().find(|&s| s > current).unwrap_or(current)
}
