# Extremely Effective Prompt (for Large Rust Systems)

Act as a **senior Rust systems engineer** performing an **abstraction-oriented code review**.

Analyze the codebase and identify places where the implementation could be **simplified, generalized, or made more idiomatic** using Rust’s abstraction mechanisms, including:

* **Generics** to eliminate repeated implementations
* **Traits** to encapsulate shared behavior
* **Trait objects (`dyn Trait`)** for runtime polymorphism where appropriate
* **Associated types** to simplify trait signatures
* **`macro_rules!` macros** to eliminate repetitive `impl` blocks or methods
* **`derive` macros** to replace manual trait implementations
* **Iterator combinators** to replace imperative loops
* **Const generics** to unify fixed-size structures
* **Blanket implementations** to generalize behavior across types

For each issue:

1. Identify the abstraction opportunity
2. Explain the benefit
3. Suggest a concrete refactoring

Prefer:

* **zero-cost abstractions**
* **idiomatic Rust patterns**
* **simpler architecture**
* **reduced boilerplate**
* Use macro_rules! only when it significantly reduces boilerplate; avoid macros that merely enumerate many similar cases—prefer direct impl blocks in such cases.

---

# Refactoring Workflow

1. **Apply all identified refactorings in a single pass.**

2. **Commit the changes.**

3. **Review the refactored code again** to identify any remaining abstraction opportunities.

4. **Apply additional refactorings if needed.**

5. **Commit the changes.**

6. **Repeat this review–refactor cycle two more times** to further improve abstraction and reduce boilerplate.

Focus on producing **cleaner, more composable, and maintainable Rust code** while preserving correctness and performance.

---

## Features and APIs Review

Review the codebase to evaluate whether the **features and public APIs are well-designed, minimal, and idiomatic for Rust**.

Identify issues such as:

* **Unnecessary or redundant features** that increase complexity
* **Overly fragmented APIs** that could be simplified or unified
* **Missing abstractions** that would improve API ergonomics
* **Leaky internal details** exposed in the public API
* **Inconsistent naming or API conventions**
* **Overly specific APIs** that should be generalized using generics or traits
* **APIs that unnecessarily restrict usage** (e.g., concrete types instead of trait bounds)
* **Public APIs that should be internal**
* **Internal APIs that should be stabilized and made public**

Evaluate whether APIs follow **idiomatic Rust patterns**, such as:

* clear ownership and borrowing semantics
* ergonomic error handling (`Result`, custom error types)
* builder patterns for complex configuration
* iterator-based APIs
* trait-based extensibility
* minimal but expressive type signatures

For each issue:

1. Identify the API or feature design problem
2. Explain why it harms usability, flexibility, or maintainability
3. Suggest a clearer or more idiomatic Rust API design.

Prefer designs that are:

* **simple**
* **composable**
* **minimal**
* **idiomatic**
