
# shift

Task time tracker written in Rust.

## Installation

```bash
git clone https://github.com/OscarCreator/shift
cd shift
cargo install --path .
```

## Examples

Start two tasks and stop one.

```bash
st start task1
st start task2
st stop task1
```

Show tasks up to 2024-02-10 02:50.

```bash
st log --all --to "2024-02-10 02:50"
```
