//! Statistics computation for volume data
//!
//! This module provides generic functions for computing statistics
//! (min, max, mean, RMS) over voxel data.

/// Statistics computed from volume data
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Statistics {
    /// Minimum value
    pub min: f64,
    /// Maximum value
    pub max: f64,
    /// Mean value
    pub mean: f64,
    /// RMS deviation
    pub rms: f64,
}

/// Compute statistics from an iterator of values
///
/// This is a generic function that works with any iterator producing
/// values that can be converted to f64.
///
/// # Type Parameters
/// * `T` - The input value type, must implement `Into<f64>`
/// * `I` - The iterator type
///
/// # Arguments
/// * `iter` - An iterator over the values
///
/// # Returns
/// A `Statistics` struct containing min, max, mean, and RMS values.
///
/// # Example
/// ```
/// use mrc::stats::{compute_stats, Statistics};
///
/// let data = vec![1.0f32, 2.0, 3.0, 4.0, 5.0];
/// let stats = compute_stats(data.into_iter());
///
/// assert_eq!(stats.min, 1.0);
/// assert_eq!(stats.max, 5.0);
/// assert_eq!(stats.mean, 3.0);
/// ```
pub fn compute_stats<T, I>(iter: I) -> Statistics
where
    T: Into<f64>,
    I: Iterator<Item = T>,
{
    let mut min = f64::INFINITY;
    let mut max = f64::NEG_INFINITY;
    let mut sum = 0.0;
    let mut sum_sq = 0.0;
    let mut count = 0usize;

    for value in iter {
        let v: f64 = value.into();
        min = min.min(v);
        max = max.max(v);
        sum += v;
        sum_sq += v * v;
        count += 1;
    }

    let n = count as f64;
    let mean = if n > 0.0 { sum / n } else { 0.0 };
    let variance = if n > 0.0 { (sum_sq / n) - (mean * mean) } else { 0.0 };
    let rms = variance.max(0.0).sqrt();

    Statistics { min, max, mean, rms }
}

/// Compute statistics from a slice
///
/// Convenience function for computing statistics from a slice of values.
///
/// # Example
/// ```
/// use mrc::stats::compute_stats_slice;
///
/// let data = [1.0f32, 2.0, 3.0, 4.0, 5.0];
/// let stats = compute_stats_slice(&data);
/// ```
pub fn compute_stats_slice<T>(slice: &[T]) -> Statistics
where
    T: Into<f64> + Copy,
{
    compute_stats(slice.iter().copied())
}

/// Compute running statistics incrementally
///
/// This struct allows computing statistics in an incremental fashion,
/// useful for streaming data or when the full dataset doesn't fit in memory.
#[derive(Debug, Clone, Copy, Default)]
pub struct RunningStats {
    count: usize,
    min: f64,
    max: f64,
    sum: f64,
    sum_sq: f64,
}

impl RunningStats {
    /// Create a new RunningStats
    pub fn new() -> Self {
        Self {
            count: 0,
            min: f64::INFINITY,
            max: f64::NEG_INFINITY,
            sum: 0.0,
            sum_sq: 0.0,
        }
    }
    
    /// Add a value to the statistics
    pub fn add<T: Into<f64>>(&mut self, value: T) {
        let v = value.into();
        self.min = self.min.min(v);
        self.max = self.max.max(v);
        self.sum += v;
        self.sum_sq += v * v;
        self.count += 1;
    }
    
    /// Add multiple values from an iterator
    pub fn extend<T, I>(&mut self, iter: I)
    where
        T: Into<f64>,
        I: Iterator<Item = T>,
    {
        for value in iter {
            self.add(value);
        }
    }
    
    /// Get the current count
    pub fn count(&self) -> usize {
        self.count
    }
    
    /// Compute the final statistics
    pub fn finish(self) -> Statistics {
        let n = self.count as f64;
        let mean = if n > 0.0 { self.sum / n } else { 0.0 };
        let variance = if n > 0.0 { (self.sum_sq / n) - (mean * mean) } else { 0.0 };
        let rms = variance.max(0.0).sqrt();
        
        Statistics {
            min: if self.count > 0 { self.min } else { 0.0 },
            max: if self.count > 0 { self.max } else { 0.0 },
            mean,
            rms,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use alloc::vec;
    use alloc::vec::Vec;
    
    #[test]
    fn test_compute_stats() {
        let data = vec![1.0f32, 2.0, 3.0, 4.0, 5.0];
        let stats = compute_stats(data.into_iter());
        
        assert_eq!(stats.min, 1.0);
        assert_eq!(stats.max, 5.0);
        assert_eq!(stats.mean, 3.0);
        assert!((stats.rms - 1.4142).abs() < 0.001); // sqrt(2)
    }
    
    #[test]
    fn test_compute_stats_empty() {
        let data: Vec<f32> = vec![];
        let stats = compute_stats(data.into_iter());
        
        assert_eq!(stats.min, f64::INFINITY);
        assert_eq!(stats.max, f64::NEG_INFINITY);
        assert_eq!(stats.mean, 0.0);
        assert_eq!(stats.rms, 0.0);
    }
    
    #[test]
    fn test_compute_stats_single() {
        let data = vec![5.0f32];
        let stats = compute_stats(data.into_iter());
        
        assert_eq!(stats.min, 5.0);
        assert_eq!(stats.max, 5.0);
        assert_eq!(stats.mean, 5.0);
        assert_eq!(stats.rms, 0.0);
    }
    
    #[test]
    fn test_running_stats() {
        let mut running = RunningStats::new();
        running.add(1.0f32);
        running.add(2.0f32);
        running.add(3.0f32);
        
        let stats = running.finish();
        assert_eq!(stats.min, 1.0);
        assert_eq!(stats.max, 3.0);
        assert_eq!(stats.mean, 2.0);
    }
    
    #[test]
    fn test_compute_stats_slice() {
        let data = [1.0f32, 2.0, 3.0, 4.0, 5.0];
        let stats = compute_stats_slice(&data);
        
        assert_eq!(stats.min, 1.0);
        assert_eq!(stats.max, 5.0);
        assert_eq!(stats.mean, 3.0);
    }
}
