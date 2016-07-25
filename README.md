Lycan
=====

Lycan is the game engine of the Renaissance project

## Start an instance

To start an instance of Lycan, you first need to start a http server to deliver
the combat.aariba file.

```bash
cd scripts
./start_server.sh &
```

Once it is done, just return to the root of the project and use Cargo

```bash
cargo run
```

## Management API

The management API is accessible on the port 9001. The Access-Token needed to authenticate is
currently hardcoded to the value "abcdefgh".
