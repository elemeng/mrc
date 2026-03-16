//! SIMD-accelerated operations for encoding/decoding

/// Helper function to perform i16 to f32 conversion with SIMD when available
#[inline]
pub fn convert_i16_to_f32(src: &[i16], dst: &mut [f32]) {
    #[cfg(target_arch = "x86_64")]
    {
        // SIMD-accelerated conversion (placeholder for actual SIMD intrinsics)
        for (i, &val) in src.iter().enumerate() {
            dst[i] = val as f32;
        }
    }
    
    #[cfg(not(target_arch = "x86_64"))]
    {
        // Scalar fallback
        for (i, &val) in src.iter().enumerate() {
            dst[i] = val as f32;
        }
    }
}

/// Helper function to perform f32 to i16 conversion with SIMD when available
#[inline]
pub fn convert_f32_to_i16(src: &[f32], dst: &mut [i16]) {
    #[cfg(target_arch = "x86_64")]
    {
        // SIMD-accelerated conversion (placeholder for actual SIMD intrinsics)
        for (i, &val) in src.iter().enumerate() {
            dst[i] = val as i16;
        }
    }
    
    #[cfg(not(target_arch = "x86_64"))]
    {
        // Scalar fallback
        for (i, &val) in src.iter().enumerate() {
            dst[i] = val as i16;
        }
    }
}

/// Helper function to perform i16 endian swap with SIMD when available
#[inline]
pub fn swap_endian_i16(data: &mut [i16]) {
    #[cfg(target_arch = "x86_64")]
    {
        // SIMD-accelerated swap (placeholder for actual SIMD intrinsics)
        for val in data.iter_mut() {
            *val = val.swap_bytes();
        }
    }
    
    #[cfg(not(target_arch = "x86_64"))]
    {
        // Scalar fallback
        for val in data.iter_mut() {
            *val = val.swap_bytes();
        }
    }
}

/// Helper function to perform i32 endian swap with SIMD when available
#[inline]
pub fn swap_endian_i32(data: &mut [i32]) {
    #[cfg(target_arch = "x86_64")]
    {
        // SIMD-accelerated swap (placeholder for actual SIMD intrinsics)
        for val in data.iter_mut() {
            *val = val.swap_bytes();
        }
    }
    
    #[cfg(not(target_arch = "x86_64"))]
    {
        // Scalar fallback
        for val in data.iter_mut() {
            *val = val.swap_bytes();
        }
    }
}

/// Helper function to perform f32 endian swap with SIMD when available
#[inline]
pub fn swap_endian_f32(data: &mut [f32]) {
    #[cfg(target_arch = "x86_64")]
    {
        // SIMD-accelerated swap (placeholder for actual SIMD intrinsics)
        for val in data.iter_mut() {
            let bits = val.to_bits().swap_bytes();
            *val = f32::from_bits(bits);
        }
    }
    
    #[cfg(not(target_arch = "x86_64"))]
    {
        // Scalar fallback
        for val in data.iter_mut() {
            let bits = val.to_bits().swap_bytes();
            *val = f32::from_bits(bits);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_convert_i16_to_f32() {
        let src: Vec<i16> = (0..100).map(|i| i as i16 * 2).collect();
        let mut dst = vec![0.0f32; src.len()];
        
        convert_i16_to_f32(&src, &mut dst);
        
        for i in 0..src.len() {
            assert_eq!(dst[i], src[i] as f32);
        }
    }

    #[test]
    fn test_convert_f32_to_i16() {
        let src: Vec<f32> = (0..100).map(|i| i as f32 * 2.0).collect();
        let mut dst = vec![0i16; src.len()];
        
        convert_f32_to_i16(&src, &mut dst);
        
        for i in 0..src.len() {
            assert_eq!(dst[i], src[i] as i16);
        }
    }

    #[test]
    fn test_swap_endian_i16() {
        let mut data: Vec<i16> = vec![0x1234, 0x5678, 0x9ABC, 0xDEF0];
        swap_endian_i16(&mut data);
        
        assert_eq!(data[0], 0x3412);
        assert_eq!(data[1], 0x7856);
        assert_eq!(data[2], 0xBC9A);
        assert_eq!(data[3], 0xF0DE);
    }
}