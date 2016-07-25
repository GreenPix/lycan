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

The management API is accessible on the port 9001. All the routes are prefixed
with `api/v1`. The `Access-Token` header is needed to authenticate. As for now,
its value is hardcoded to `abcdefgh`. Hence, a valid example of request is:

```bash
curl localhost:9001/api/v1/players -H "access-token: abcdefgh"
```

Several scripts can be found in the `scripts/` directory. They can be use by a
developer to query the API more easily.
