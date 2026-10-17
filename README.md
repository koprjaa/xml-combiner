# xml-combiner

Merges a folder of XML files into one document.

[![CI](https://github.com/koprjaa/xml-combiner/actions/workflows/ci.yml/badge.svg)](https://github.com/koprjaa/xml-combiner/actions/workflows/ci.yml)
[![MIT license](https://img.shields.io/badge/license-MIT-A31F34?style=flat-square)](LICENSE)

## What it does

- Keeps each input root and its descendants under a shared wrapper.
- Preserves XML namespaces and accepts files with multiple root elements.
- Can search subdirectories and skip duplicate elements.
- Writes UTF-8 output and continues past malformed input files.

## Install

Install a current stable Rust toolchain, then build from the repository directory:

```bash
cargo build --release
```

The executable is `target/release/xml-combiner` (`target/release/xml-combiner.exe` on Windows). No environment variables are required.

## Use

Combine the `.xml` files directly inside `./xmls`:

```bash
./target/release/xml-combiner ./xmls
```

This writes `combined.xml` in the working directory, with a wrapper named `combined`. File discovery is case insensitive and sorted by path.

To collect direct children from each input root, search subdirectories, and remove duplicates:

```bash
./target/release/xml-combiner ./feeds -o all-items.xml --recursive --flatten --deduplicate --root-element items
```

With `--flatten`, root text is discarded; child text remains. Without it, deduplication compares complete input roots.

Keep the output outside the input directory so later runs do not include it. Relative output paths must stay inside the working directory. The tool creates parent directories as needed. Keep inputs unchanged while it runs: each file is read twice.

See the remaining options with:

```bash
./target/release/xml-combiner --help
```

## License

[MIT](LICENSE)
