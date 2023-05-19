# hibp-json

Turns [HaveIBeenPwned] (HIBP) passsword hash files into JSON format, along with compressing them (optionally) into `.gz` and/or `.br` files for quick static serving.

The purpose of this is to easily turn the password hash lookup service into a self-hosted instance that simply serves static files.

[HaveIBeenPwned]: https://haveibeenpwned.com/

## Prerequisites

You must have aquired all of the HIBP hash files through the [PwnedPasswordsDownloader] using the individual hash files option. This is their example of how to use their downloader that downloads into individual hash files (`-s false`) with 64 threads (`-p 64`). This step will take a while, and you should take care not to delete the created directory otherwise you will have to re-download everything.

```
haveibeenpwned-downloader.exe hashes -s false -p 64
```

[PwnedPasswordsDownloader]: https://github.com/HaveIBeenPwned/PwnedPasswordsDownloader

## Running

By default `hibp-json` expects the hashes to be in `hashes/` and the output to be created in `dist/`. This is configurable, see `hibp-json --help`.

`.json` files, `.json.gz` files, and `.json.br` files will be created. Each of these can be turned off, see `hibp-json --help`.

## Size

Here are the size of the raw files. Notably, the original format is about as efficient as possible (the first 5 chars of the hash being excluded because its in the filename) and each line is just `{hash}:{count}`. Because of this, the JSON size is somewhat larger because the full hash is included to prevent needing to remember to concat the hashes on the frontend alongside each item becoming a JSON object with `hash` and `count` fields.

This is not a huge deal, because the compression algorithms are able to cut down on the repetitive structure of the JSON, bringing the compressed versions to the size of the original version when compressed. e.g. `000D0.txt` is `35,905` bytes raw and `17,190` bytes (brotli) compressed while `000D0.json` is `57,045` bytes raw and `17,405` bytes (brotli) compressed.

Thus, if you decide to **only** serve compressed assets, then you don't need the raw JSON files and can exclude them from generation by adding `--json false`. This allows you to only serve gzip or brotli compressed files, and possibly optionally decompressing on-the-fly when requested for a non-compressed file. Virtually every browser and most tools support gzip compressed content, and a [very large amount (96.6%) of browsers support brotli](https://caniuse.com/brotli).

| Source | Size | Time |
| ------ | ---- | ---- |
| Original | ~`33 GiB` | n/a |
| JSON | ~`50 GiB` | `3m` |
| Gzip | ~`17.3 GiB` | `5m` |
| Brotli | ~`15.3 GiB` | `90m` |

<sup>note: timings taken on my Macbook Pro m2 Pro</sup>

I suggest only choosing a single compressed version based on your requirements and excluding generating raw JSON files. Brotli has slightly lower global support (but still extremely high) and takes consideribly longer to generate but is `69.39%` smaller compared to gzip at `65.52%`.

## Examples

This show generating only json, json + gz, json + br, and json + gz + br in order. These examples are only using the first 2048 hashes, and therefore need to pass `--strict false` to prevent exiting due to unexpected input size.

```console
chip@cancer hibp-json % rm -rf dist && /usr/bin/time ./target/release/hibp-json --strict false --brotli false --gzip false
[1/3] Ensured 65,536 output directories in 1709ms
[2/3] Found 2048 hash files in benchdata in 1ms
[3/3] Generating .json files 
Finished generating files in 68ms (1778ms total)
Bytes: json 105166422 | br 0 | gz 0
        1.78 real         0.22 user         1.87 sys

chip@cancer hibp-json % rm -rf dist && /usr/bin/time ./target/release/hibp-json --strict false --brotli false             
[1/3] Ensured 65,536 output directories in 1626ms
[2/3] Found 2048 hash files in benchdata in 2ms
[3/3] Generating .json .gz files 
Finished generating files in 354ms (1983ms total)
Bytes: json 105166422 | br 0 | gz 36263187
        1.98 real         2.80 user         2.08 sys

chip@cancer hibp-json % rm -rf dist && /usr/bin/time ./target/release/hibp-json --strict false --gzip false 
[1/3] Ensured 65,536 output directories in 1644ms
[2/3] Found 2048 hash files in benchdata in 4ms
[3/3] Generating .json .br files 
Finished generating files in 10536ms (12186ms total)
Bytes: json 105166422 | br 32191367 | gz 0
       12.19 real       112.09 user         2.70 sys

chip@cancer hibp-json % rm -rf dist && /usr/bin/time ./target/release/hibp-json --strict false               
[1/3] Ensured 65,536 output directories in 1758ms
[2/3] Found 2048 hash files in benchdata in 4ms
[3/3] Generating .json .br .gz files 
Finished generating files in 10817ms (12580ms total)
Bytes: json 105166422 | br 32191367 | gz 36263187
       12.59 real       114.42 user         3.05 sys
```

## Server

There is a server located in [`server/`](server). It is currently someone rudimentary. Run it inside the `dist/` directory, or set the location with the `ROOT` env var. Then check the server index for instructions for how to use the API. (default http://127.0.0.1:8080/)

It currently supports:
* `ROOT` env var to set the path to the `dist/` directory created from the tool (defaults to current working directory)
* `HOST` to set the host to something other than `127.0.0.1`
* `PORT` to set the port to something other than `8080`
* Checks compression (and raw json) support by just reading the `$ROOT/0/0/0/0/0.json` file (and `.br`, `.gz`)
* Simple parser for `Accept-Encoding` that completely ignores weight

TODO:
* Comprehensive validation of the `ROOT` directory, including compression support
* Decompression on-the-fly if wanted if `.json` files don't exist (compile-time feature?)
* Improve error messages
* Cleanup code a bit

#### License

<sup>
Licensed under either of <a href="LICENSE-APACHE">Apache License, Version
2.0</a> or <a href="LICENSE-MIT">MIT license</a> at your option.
</sup>

<br>

<sub>
Unless you explicitly state otherwise, any contribution intentionally submitted
for inclusion in this crate by you, as defined in the Apache-2.0 license, shall
be dual licensed as above, without any additional terms or conditions.
</sub>
