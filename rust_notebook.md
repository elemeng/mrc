# Rust Code Notebook (Refined)

## The Core Mental Shift: From "Objects" to "Data Flow"

**Python thinks:** "Objects have methods"  
**Rust thinks:** "Functions operate on data with capabilities"

---

## Mental Checklist (Corrected)

When you see a function: `fn foo<T: Read + Seek>(source: &mut T) -> Result<(), Error>`

**Ask these four questions:**

1. **Data:** What data do I have? → `source` is a mutable reference to type `T`
2. **Capabilities:** What can this data do? → `T` implements `Read` and `Seek` traits  
3. **Flow:** How does data flow? → `source` will be read and seeked, but **not** consumed (you keep ownership)
4. **Use next:** What do I use after the call?  
   - `()` return → use **modified parameter** or **modified receiver**
   - `T` return → use **returned value** (bind to variable)

---

## Function Syntax

```rust
fn function_name(parameter_name: ParameterType) -> ReturnType {
    // function body
}
```

- **Parameters** require explicit types (no inference for fn signatures)
- **Return type** is always explicit (can be `()` for "nothing")

---

## Common Patterns by Return Type

### Pattern 1: `Result<(), E>` + `&mut param` → **Use the parameter**

```rust
// Signature
fn read_exact(&mut self, buf: &mut [u8]) -> Result<(), Error>

// Usage
let mut buffer = [0u8; 1024];
file.read_exact(&mut buffer)?;  // Returns Ok(()) - ignore it
// USE: buffer (now filled with data)
```

### Pattern 2: `Result<T, E>` → **Use the return value**

```rust
// Signature
fn parse<F>(&self) -> Result<F, F::Err>

// Usage
let value = "42".parse::<u32>()?;  // Returns Ok(42) or Err
// USE: value (the returned u32)
```

### Pattern 3: `&mut self` + `()` → **Use the receiver**

```rust
// Signature
fn push(&mut self, value: T) -> ()

// Usage
vec.push(42);  // Returns ()
// USE: vec (now contains 42)
```

---

## File I/O Operations

```rust
/// Opens a file handle for reading (does NOT read any bytes yet)
/// Returns a Result<File, Error>
let mut file = File::open(path)?;

/// Creates a stack-allocated array of 1024 bytes, all initialized to 0 of type u8
/// Syntax: [value; count]
let mut header_bytes = [0u8; 1024];  // Array, NOT an iterator!

/// The read pointer starts at position 0 for newly opened files
/// BUT: Always seek explicitly for clarity and robustness
file.seek(SeekFrom::Start(0))?;  // Position = 0

/// Reads EXACTLY 1024 bytes from CURRENT position into buffer
/// Advances file position by 1024 bytes
file.read_exact(&mut header_bytes)?;
```

**Key principle:** `seek` controls position, `read` reads from that position into your buffer.

---

## Method Call Syntax: `receiver.method(arguments)`

```
receiver.method_name(arg1, arg2, ...)
│      │            │
│      │            └─ Arguments: Inputs to the method
│      └─ Method name: Which operation to perform
└─ Receiver: The data being operated on (can be self, &self, &mut self)
```

**Critical correction:** The receiver does **NOT** become the return value. They are separate:

- `receiver` may be **modified** (if `&mut self`)
- `return value` is **separate data** (could be `()`, `T`, or `Result<T, E>`)

### Receiver Types Determine Usage

```rust
impl File {
    fn do_something(&self)    {}  // Receiver: immutable borrow
    fn modify_self(&mut self) {}  // Receiver: mutable borrow (self changes)
    fn consume_self(self)     {}  // Receiver: owned (self is consumed/moved)
}
```

---

## Traits: The "Capability" Model

**Trait = What you can DO with data**

```rust
// File has these capabilities
impl Read for File { }   // ✅ Can read bytes
impl Seek for File { }   // ✅ Can seek positions
impl Write for File { }  // ✅ Can write bytes

// Vec has these capabilities
impl Read for Vec<u8> { }  // ✅ Can read (from memory)
// ❌ NO Seek - you can't "seek" in memory

// String has this capability
impl Display for String { }  // ✅ Can be displayed/printed
```

### Trait Bound Syntax

```rust
fn process<T: Read + Seek>(source: &mut T) -> Result<(), Error> {
    // Compiler guarantees source can read AND seek
    source.seek(SeekFrom::Start(0))?;
    source.read(&mut buf)?;
    Ok(())
}
```

**Mental model:** "I need data that can **read** and **seek**" (not "I need a File object")

---

## The Four Questions: A Practical Workflow

When encountering any function/method:

```rust
file.read_exact(&mut buffer)?;  // Break it down:

1. Data:       &mut file, &mut buffer
2. Capability: File implements Read trait
3. Flow:       file → buffer (source to destination)
4. Use next:   Returns () → use buffer
```

---

## Common Pitfalls

- **`[0u8; 1024]`** is an **array**, not an iterator
- **Receiver ≠ Return value** - they are separate pieces of data
- **Always check `&mut`** - that's what gets modified
- **`()` return** means the useful data is in parameters/receiver
- **Skip seek only** if you explicitly document the precondition

---

## Official Documentation Priority

1. **Hover** in VS Code (quick type info)
2. **`F12` Go to Definition** (see actual signature)
3. **docs.rust-lang.org** (full docs with examples)
4. **Look for `&mut`** in signature (what gets modified)
5. **Check return type** first (what to use next)
