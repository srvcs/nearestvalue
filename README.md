# srvcs-nearestvalue

The nearest-value range service of the srvcs.cloud distributed standard library.

Its single concern: **which element of a list of integers is nearest to a
reference value?** It returns the element minimizing the absolute difference to
`value`; on a tie, the element that appears first wins.

`srvcs-nearestvalue` is a **leaf**: it depends on no other service and makes no
network calls. All work is local.

```text
result = element of values minimizing (element - value).abs()   (first on ties)
```

## API

| Method | Path | Purpose |
| --- | --- | --- |
| `GET` | `/` | Service identity, concern, and dependency list |
| `POST` | `/` | Return the element of `values` nearest to `value` |
| `GET` | `/healthz` `/readyz` `/metrics` `/openapi.json` | srvcs service standard surface |

```sh
curl -s -X POST localhost:8080/ -H 'content-type: application/json' -d '{"value": 5, "values": [1, 4, 9]}'
# {"value":5,"values":[1,4,9],"result":4}

curl -s -X POST localhost:8080/ -H 'content-type: application/json' -d '{"value": 7, "values": [1, 4, 9]}'
# {"value":7,"values":[1,4,9],"result":9}
```

Responses:

- `200 {"value": int, "values": [int, ...], "result": int}` — evaluated.
  `result` is the element of `values` nearest to `value`.
- `422 {"error": ...}` — `value` or some element of `values` is not a JSON
  integer, or `values` is empty.

Distances are computed as `(element - value).abs()`. When two elements are
equally near, the one that appears earliest in `values` is returned.

## Dependencies

None. `srvcs-nearestvalue` is a leaf range service. Because it owns its own
validation, it rejects any non-integer input or empty list directly with `422`
rather than forwarding to a dependency.

## Configuration

| Variable | Default | Purpose |
| --- | --- | --- |
| `SRVCS_BIND_ADDR` | `0.0.0.0:8080` | Bind address |
| `SRVCS_ENV` | `development` | Environment label for logs |
| `RUST_LOG` | `info,tower_http=info` | Tracing filter |

## Local checks

```sh
cargo fmt --check
cargo clippy --all-targets -- -D warnings
cargo test
```

See [`srvcs/platform`](https://github.com/srvcs/platform) for the shared
standard.

> Note: the `cargoHash` in `flake.nix` is inherited from the template and must be
> refreshed with a `nix build` before the Nix gates pass.
