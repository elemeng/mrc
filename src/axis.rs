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
    pub column: usize,
    /// Row axis
    pub row: usize,
    /// Section axis (slowest varying in memory)
    pub section: usize,
}

impl Default for AxisMap {
    #[inline]
    fn default() -> Self {
        Self::STANDARD
    }
}

impl AxisMap {
    /// Standard axis ordering (X=1, Y=2, Z=3)
    pub const STANDARD: Self = Self { column: 1, row: 2, section: 3 };
    
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
    #[inline]
    pub fn axis_index(&self, dim: usize) -> usize {
        match dim {
            0 => self.column - 1, // X
            1 => self.row - 1,    // Y
            2 => self.section - 1, // Z
            _ => panic!("Invalid dimension: {dim}"),
        }
    }
    
    /// Get stride multipliers for indexing
    ///
    /// Returns (stride_x, stride_y, stride_z) for computing linear indices
    pub fn strides(&self, shape: [usize; 3]) -> [usize; 3] {
        let nx = shape[0];
        let ny = shape[1];
        let nz = shape[2];
        
        // Map from logical (x, y, z) to storage order
        let stride_x = match self.column {
            1 => 1,           // X is column
            2 => nx,          // X is row
            3 => nx * ny,     // X is section
            _ => unreachable!(),
        };
        
        let stride_y = match self.row {
            1 => 1,
            2 => nx,
            3 => nx * ny,
            _ => unreachable!(),
        };
        
        let stride_z = match self.section {
            1 => 1,
            2 => nx,
            3 => nx * ny,
            _ => unreachable!(),
        };
        
        [stride_x, stride_y, stride_z]
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
