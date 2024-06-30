# shift (st)

shift is a time tracker written in Rust and designed to be simple but effective
to use for tracking time between projects. Simply start a task with `st start task1`
and once finished use `st stop`. shift stores each of these events in a sqlite3 database
which is by default located at `$XDG_CONFIG_HOME/.local/share/st/events.db` or `$HOME/.local/share/st/events.db`.


### Installation

```bash
git clone https://github.com/OscarCreator/shift
cd shift
cargo install --path .
```

### Examples

shift has a few commands to 

Show all available commands.
```bash
st help
```

Start two tasks and stop one. If only one task in ongoing then no need to specify
which task to stop.
```bash
st start task1
st start task2
st stop task1
```

Pause ongoing task(s) for later resuming. This works well if you have multiple
tasks ongoing and will just take a short break `st pause --all` and then resume
with `st resume --all`.
```bash
st start task1
st pause
st resume
```

Show events up to 2024-02-10 02:50.
```bash
st log --all --to "2024-02-10 02:50"
```

#### Inspired by
* [Watson](https://github.com/TailorDev/Watson)

