# Contributing to Vajra

First off, thank you for considering contributing to Vajra! It's people like you that make building open-source tools such a rewarding experience.

## Why I built this

I built Vajra strictly out of curiosity and a deep desire to learn systems programming from the ground up. I wanted to demystify the magic behind distributed databases and modern AI infrastructure. Instead of just using existing tools like Milvus or Qdrant, I wanted to understand the raw mechanics of consensus algorithms, write-ahead logs, and vector indexes. Rust provided the perfect playground—demanding rigor while empowering performance. What started as late-night tinkering with Raft and HNSW eventually became this project; a testament to breaking down black boxes and building them back up from first principles.

## Getting Started

1. Fork the repository and clone it locally.
2. Ensure you have Rust and Cargo installed (`1.75+` is recommended).
3. Run `cargo test` to verify everything builds properly.
4. Check out the architecture details in `README.md` to get a feel for how the HNSW index, Raft consensus, and WAL interlock.

## Pull Request Guidelines

- Keep PRs focused on a single change, fix, or feature.
- Ensure all tests pass. If you're adding a core feature, please include tests!
- Format code with `cargo fmt` before submitting.
- Document any significant architectural changes.

I am excited to see your ideas, feedback, and improvements. Happy coding!
