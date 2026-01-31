# cfg(debug_assertions) ( Custom Debug Blocks )

Wrap any code (prints, asserts, expensive checks) to only compile in debug.

```rust
// In lib.rs, DataBlock::read_f32_into()
pub fn read_f32_into(&self, out: &mut [f32]) -> Result<(), Error> {
    // This entire block disappears in release builds
    #[cfg(debug_assertions)]
    {
        println!("[DEBUG] Decoding {} f32 values", out.len());
        println!("[DEBUG] File endian: {:?}", self.file_endian);

        // Custom assertion (only runs in debug)
        assert!(
            out.len() * 4 == self.bytes.len(),
            "Buffer size mismatch: {} vs {}", out.len() * 4, self.bytes.len()
        );
        
        // Even expensive validation
        if self.bytes.len() > 1_000_000 {
            eprintln!("[WARN] Large data block: {} MB", self.bytes.len() / 1_000_000);
        }
    }

    // Release-mode code continues here
    if self.file_endian.is_native() {
        // ... actual decoding
    }
}
```

What happens:
✅ Debug mode: All code inside runs normally
✅ Release mode: Entire block is compiled out (like it never existed)
✅ Zero performance cost in release
Best for: Expensive validation, debug-only logic, temporary experiments
