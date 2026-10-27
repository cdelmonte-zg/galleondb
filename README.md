# GalleonDB

> “Fatti non foste a viver come bruti,  
> ma per seguir virtute e canoscenza.”
>
> — Dante Alighieri, *Inferno*, XXVI

My little miniature galleon of a database engine.

Built in Rust, just to understand what actually happens underneath SQL: parsing, planning, execution, pages, records, B-trees and indexes.

Started from the CodeCrafters **Build your own SQLite** challenge and continued as a personal project for exploring database internals.

It currently reads SQLite databases and supports a small subset of SQL, including indexed lookups.

```text
SQL
 ↓
Scanner
 ↓
Parser
 ↓
Resolver
 ↓
Logical Plan
 ↓
Executor
 ↓
SQLite B-trees
```

Not a production database. Not trying to be one.

Just a small ship to explore database internals and play around.

## Run

```bash
cargo build
cargo test
```