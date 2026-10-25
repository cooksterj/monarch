# Attribution

The agent-detection manifests in this directory (`*.toml`) come unmodified from
[Herdr](https://github.com/herdrdev/herdr) (`src/detect/manifests/`, commit
`5608de0608918aba2f9d501e7793b984a0ca9cfa`), copyright the Herdr contributors and licensed
under the [Apache License, Version 2.0](https://www.apache.org/licenses/LICENSE-2.0).

Monarch reads them with its own engine (`monarch-detect`), which follows Herdr's manifest
format: the same header, rules, regions and gates.

To update them, copy the files from a newer Herdr commit, record that commit here, and run
`cargo test -p monarch-mux-detect`.
