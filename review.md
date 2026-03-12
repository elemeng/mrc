

   1. `Packed4Bit` unsafe code - Special case in Volume::set() uses unsafe pointer cast when it could use the Encoding trait.

   2. `VolumeData` downcast methods - 8 repetitive methods that could be generated with a macro.
