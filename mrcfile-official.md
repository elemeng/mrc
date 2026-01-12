## MRC/CCP4 2014 file format

This page gives the detailed specification of the MRC2014 format, as published by [Cheng et al. (2015)](https://doi.org/10.1016/j.jsb.2015.04.002). For other versions of the MRC format and an overview of the format update process, see the [parent page](https://www.ccpem.ac.uk/mrc-format).

CCP-EM maintain a [Python library](https://mrcfile.readthedocs.io/en/stable/index.html) for reading, writing and validating MRC2014 files.

## Main header

Length = 1024 bytes, organized as 56 4-byte words followed by space for 10 80-byte text labels.

| **Word** | **Bytes** | **Variable name**[†](https://www.ccpem.ac.uk/mrc-format/mrc2014/#%E2%80%A0) | Description | **Note** |
| --- | --- | --- | --- | --- |
| 1 | 1-4 | NX or NC | number of columns in 3D data array (fast axis) | [1](https://www.ccpem.ac.uk/mrc-format/mrc2014/#1) |
| 2 | 5-8 | NY or NR | number of rows in 3D data array (medium axis) |  |
| 3 | 9-12 | NZ or NS | number of sections in 3D data array (slow axis) |  |
| 4 | 13-16 | MODE | **0** 8-bit signed integer (range -128 to 127)   **1** 16-bit signed integer   **2** 32-bit signed real   **3** transform : complex 16-bit integers   **4** transform : complex 32-bit reals   **6** 16-bit unsigned integer   **12** 16-bit float (IEEE754)   **101** 4-bit data packed two per byte | [2](https://www.ccpem.ac.uk/mrc-format/mrc2014/#2) |
| 5 | 17-20 | NXSTART or NCSTART | location of first column in unit cell |  |
| 6 | 21-24 | NYSTART or NRSTART | location of first row in unit cell |  |
| 7 | 25-28 | NZSTART or NSSTART | location of first section in unit cell |  |
| 8 | 29-32 | MX | sampling along X axis of unit cell |  |
| 9 | 33-36 | MY | sampling along Y axis of unit cell |  |
| 10 | 37-40 | MZ | sampling along Z axis of unit cell | [3](https://www.ccpem.ac.uk/mrc-format/mrc2014/#3) |
| 11-13 | 41-52 | CELLA | cell dimensions in angstroms |  |
| 14-16 | 53-64 | CELLB | cell angles in degrees |  |
| 17 | 65-68 | MAPC | axis corresp to cols (1,2,3 for X,Y,Z) | [4](https://www.ccpem.ac.uk/mrc-format/mrc2014/#4) |
| 18 | 69-72 | MAPR | axis corresp to rows (1,2,3 for X,Y,Z) |  |
| 19 | 73-76 | MAPS | axis corresp to sections (1,2,3 to X,Y,Z) |  |
| 20 | 77-80 | DMIN | minimum density value | [5](https://www.ccpem.ac.uk/mrc-format/mrc2014/#5) |
| 21 | 81-84 | DMAX | maximum density value |  |
| 22 | 85-88 | DMEAN | mean density value |  |
| 23 | 89-92 | ISPG | space group number | [6](https://www.ccpem.ac.uk/mrc-format/mrc2014/#6) |
| 24 | 93-96 | NSYMBT | size of extended header (which follows main header) in bytes | [7](https://www.ccpem.ac.uk/mrc-format/mrc2014/#7) |
| 25-49 | 97-196 | EXTRA | extra space used by anything – 0 by default |  |
| 27 | 105 | EXTTYP | code for the type of extended header | [8](https://www.ccpem.ac.uk/mrc-format/mrc2014/#8) |
| 28 | 109 | NVERSION | version of the MRC format | [9](https://www.ccpem.ac.uk/mrc-format/mrc2014/#9) |
| 50-52 | 197-208 | ORIGIN | phase origin (pixels) or origin of subvolume (A) | [10](https://www.ccpem.ac.uk/mrc-format/mrc2014/#10) |
| 53 | 209-212 | MAP | character string ‘MAP’ to identify file type |  |
| 54 | 213-216 | MACHST | machine stamp encoding byte ordering of data | [11](https://www.ccpem.ac.uk/mrc-format/mrc2014/#11) |
| 55 | 217-220 | RMS | rms deviation of map from mean density |  |
| 56 | 221-224 | NLABL | number of labels being used |  |
| 57-256 | 225-1024 | LABEL(20,10) | 10x 80 character text labels |  |

## Extended header

In the original definition, the extended header holds space group symmetry records stored as text as in International Tables, operators separated by \* and grouped into ‘lines’ of 80 characters (ie. symmetry operators do not cross the ends of the 80-character ‘lines’ and the ‘lines’ do not terminate in a \*). The extended header is now used by different software to hold various additional metadata instead, as indicated by the EXTTYP tag.

## Data block

A list of data values representing the image/map/volume itself. The data type is defined by the MODE keyword in the main header (see above). The data items form a 3-dimensional grid, organised into columns, rows and sections (see keywords NX, NY, NZ in main header). The orientation of the grid with respect to the coordinate system is set by keywords MAPC, MAPR, MAPS in the main header. The 3-dimensional grid may represent a stack of images or a stack of volumes, see note above.

### Handedness

The handedness of the data block is not well defined by the MRC2014 standard. Conventionally, many pieces of software have treated the data as right-handed, with the origin in the bottom left corner of a 2D image and the Z-axis pointing out of the screen.

However, this approach is not universal, and some packages treat the data block as left-handed. An example is FEI’s EPU data acquisition software, which places the image origin in the top left, as documented in [appendix C of the EPU User Manual](https://www.ccpem.ac.uk/downloads/other/EPU_user_manual_AppendixCfordistribution.pdf).

Proposals for indicating the data handedness in the file header are under discussion, but for now, the only way to be sure of the handedness is to check the behaviour of each software package individually.

## Notes

Note †

The variable name is not used in the file format and so is arbitrary. There are standard names that are typically encountered in software and documentation, and we list the common ones. Often they are referred to differently in EM and crystallography.

Note 1

The data block of an MRC format file holds a 3D array of data (of type specified by MODE). NC, NR, NS specify the dimensions (in grid points) of this array, orientated according to MAPC/MAPR/MAPS. In EM, this will correspond to the dimensions of a volume/map, or the combined size of an image/volume stack. In crystallography, this will correspond to the dimensions of a map, which may cover a crystallographic unit cell or may cover some fraction or multiple of a unit cell.

Note 2

In the MRC2014 format, Mode 0 has been clarified as signed, and mode 6 has been added for 16-bit unsigned integer data. See [updates page](https://www.ccpem.ac.uk/mrc-format/mrc-format-proposals/) for additional modes.

Note 3

In crystallographic usage, MZ represents the number of intervals, or sampling grid, along Z in a crystallographic unit cell. This need not be the same as NZ (or NX/NY if axes permuted) if the map doesn’t cover exactly a single unit cell. For microscopy, where there is no unit cell, MZ represents the number of sections in a single volume. For a volume stack, NZ/MZ will be the number of volumes in the stack. For images, MZ = 1.

Note 4

In EM MAPC,MAPR,MAPS = 1,2,3 so that sections and images are perpendicular to the Z axis. In crystallography, other orderings are possible. For example, in some spacegroups it is convenient to section along the Y axis (i.e. where this is the polar axis).

Note 5

Density statistics may not be kept up-to-date for image/volume stacks, since it is expensive to recalculate these every time a new image/volume is added/deleted. We have proposed the following convention: DMAX < DMIN, DMEAN < (smaller of DMIN and DMAX), RMS < 0 each indicate that the quantity in question is not well determined.

Note 6

Spacegroup 0 implies a 2D image or image stack. For crystallography, ISPG represents the actual spacegroup. For single volumes from EM/ET, the spacegroup should be 1. For volume stacks, we adopt the convention that ISPG is the spacegroup number + 400, which in EM/ET will typically be 401.

Note 7

NSYMBT specifies the size of the extended header in bytes, whether it contains symmetry records (as in the original format definition) or any other kind of additional metadata.

Note 8

A code for the kind of metadata held in the extended header. Currently agreed values are:

| **CCP4** | Format from CCP4 suite |
| --- | --- |
| **MRCO** | MRC format |
| **SERI** | SerialEM. Details in the [IMOD documentation](http://bio3d.colorado.edu/imod/doc/mrc_format.txt). |
| **AGAR** | Agard |
| **FEI1** & **FEI2** | Used by Thermo Scientific and FEI software, e.g. EPU and Xplore3D, Amira, Avizo. Details can be found in this [specification document](https://www.ccpem.ac.uk/downloads/other/EPU_MRC2014_File_Image_Format_Specification_-_306687.pdf), which is also available from the [ThermoFisher software center](https://assets.thermofisher.cn/TFS-Assets/MSD/Support-Files/mrc2014-file-format-306687.pdf). |
| **HDF5** | Metadata in HDF5 format |

Note 9

The version of the MRC format that the file adheres to, specified as a 32-bit integer and calculated as:  
     Year \* 10 + version within the year (base 0)  
For the original MRC2014 format, the value was 20140, while the [latest update](https://www.ccpem.ac.uk/mrc-format/mrc-format-proposals/) is 20141.

Note 10

For transforms (Mode 3 or 4), ORIGIN is the phase origin of the transformed image in pixels, e.g. as used in helical processing of the MRC package. For a transform of a padded image, this value corresponds to the pixel position in the padded image of the centre of the unpadded image.

![](https://www.ccpem.ac.uk/wp-content/uploads/2024/06/image_real_origin_definition_v2.png)

For other modes, ORIGIN specifies the real space location of a subvolume taken from a larger volume. In the (2-dimensional) example shown above, the header of the map containing the subvolume (red rectangle) would contain ORIGIN = 100, 120 to specify its position with respect to the original volume (assuming the original volume has its own ORIGIN set to 0, 0).

Note 11

Bytes 213 and 214 contain 4 \`nibbles’ (half-bytes) indicating the representation of float, complex, integer and character datatypes. Bytes 215 and 216 are unused. The CCP4 library contains a general representation of datatypes, but in practice it is safe to use 0x44 0x44 0x00 0

---

Clipped from [https://www.ccpem.ac.uk/mrc-format/mrc2014/](https://www.ccpem.ac.uk/mrc-format/mrc2014/)
