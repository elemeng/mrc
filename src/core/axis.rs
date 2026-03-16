//! Axis mapping for MRC files
//!
//! MRC files can have permuted axes via MAPC/MAPR/MAPS fields.
//! This module handles the mapping between file order and logical XYZ.

use core::fmt;

/// Axis permutation from file storage to logical coordinates
///
/// MRC headers contain MAPC, MAPR, MAPS which specify:
/// - Which axis is the column (fastest varying): MAPC
/// - Which axis is the row: MAPR
/// - Which axis is the section (slowest varying): MAPS
///
/// Standard ordering is MAPC=1, MAPR=2, MAPS=3 (column=X, row=Y, section=Z)
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AxisMap {
    /// Column axis (fastest varying in memory)
    column: usize,
    /// Row axis
    row: usize,
    /// Section axis (slowest varying in memory)
    section: usize,
}

impl Default for AxisMap {
    #[inline]
    fn default() -> Self {
        Self::STANDARD
    }
}

impl AxisMap {
    /// Standard axis ordering (X=1, Y=2, Z=3)
    pub const STANDARD: Self = Self {
        column: 1,
        row: 2,
        section: 3,
    };

    /// Get the column axis (fastest varying in memory)
    #[inline]
    pub const fn column(&self) -> usize {
        self.column
    }

    /// Get the row axis
    #[inline]
    pub const fn row(&self) -> usize {
        self.row
    }

    /// Get the section axis (slowest varying in memory)
    #[inline]
    pub const fn section(&self) -> usize {
        self.section
    }

    /// Create from MAPC, MAPR, MAPS values without validation
    #[inline]
    pub const fn new(mapc: i32, mapr: i32, maps: i32) -> Self {
        Self {
            column: mapc as usize,
            row: mapr as usize,
            section: maps as usize,
        }
    }

    /// Create from MAPC, MAPR, MAPS values with validation
    ///
    /// Returns error if values are not a valid permutation of 1, 2, 3
    pub fn try_new(mapc: i32, mapr: i32, maps: i32) -> Result<Self, crate::Error> {
        let map = Self::new(mapc, mapr, maps);
        if map.validate() {
            Ok(map)
        } else {
            Err(crate::Error::InvalidAxisMap)
        }
    }

    /// Check if this is standard ordering
    #[inline]
    pub fn is_standard(&self) -> bool {
        *self == Self::STANDARD
    }

    /// Validate the axis map is a valid permutation of 1, 2, 3
    pub fn validate(&self) -> bool {
        let axes = [self.column, self.row, self.section];
        axes.iter().all(|&a| (1..=3).contains(&a))
            && axes[0] != axes[1]
            && axes[1] != axes[2]
            && axes[0] != axes[2]
    }

    /// Get the axis index for a given dimension (0=X, 1=Y, 2=Z)
    ///
    /// Returns `None` for invalid dimension values.
    #[inline]
    pub fn axis_index(&self, dim: usize) -> Option<usize> {
        match dim {
            0 => Some(self.column - 1),  // X
            1 => Some(self.row - 1),     // Y
            2 => Some(self.section - 1), // Z
            _ => None,
        }
    }

    /// Get stride multipliers for indexing
    ///
    /// Returns [stride_x, stride_y, stride_z] for computing linear indices
    pub fn strides(&self, dimensions: [usize; 3]) -> [usize; 3] {
        let nx = dimensions[0];
        let ny = dimensions[1];

        // Pre-compute stride options: [column, row, section]
        // Column has stride 1, row has stride nx, section has stride nx*ny
        let stride_options = [1, nx, nx * ny];

        // Map axis values (1,2,3) to stride indices (0,1,2)
        let idx = |axis: usize| axis.saturating_sub(1).min(2);

        [
            stride_options[idx(self.column)],
            stride_options[idx(self.row)],
            stride_options[idx(self.section)],
        ]
    }
}

impl fmt::Display for AxisMap {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "AxisMap(column={}, row={}, section={})",
            self.column, self.row, self.section
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_axis_index() {
        let map = AxisMap::STANDARD;
        assert_eq!(map.axis_index(0), Some(0)); // X -> column-1 = 0
        assert_eq!(map.axis_index(1), Some(1)); // Y -> row-1 = 1
        assert_eq!(map.axis_index(2), Some(2)); // Z -> section-1 = 2
        assert_eq!(map.axis_index(3), None); // Invalid
    }

    #[test]
    fn test_validation() {
        assert!(AxisMap::STANDARD.validate());
        assert!(AxisMap::try_new(1, 2, 3).is_ok());
        assert!(AxisMap::try_new(3, 2, 1).is_ok());
        assert!(AxisMap::try_new(1, 1, 3).is_err()); // Duplicate
        assert!(AxisMap::try_new(0, 2, 3).is_err()); // Out of range
    }
}
