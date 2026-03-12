   1. `dimensions()` vs `dimensions()` - Volume has both methods returning the same info in different formats. Should pick
      one.

   2. `DataBlock` vs `Volume` - They serve similar purposes (typed access to voxel data). Consider if both are needed
      or if they can be unified.

   3. `as_slice()` returns wrong error - Returns InvalidAxisMap for non-contiguous data when it should be something
      else.

   4. `Packed4Bit` unsafe code - Special case in Volume::set() uses unsafe pointer cast when it could use the
      Encoding trait.

   5. `VolumeData` downcast methods - 8 repetitive methods that could be generated with a macro.
