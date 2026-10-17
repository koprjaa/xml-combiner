<!--
Project: xml-combiner
File: README.md
Description: Installation, usage, compatibility, and development instructions.
Author: Jan Alexandr Kopřiva
License: MIT
-->

# xml-combiner

Merges a directory of XML files into one file. It keeps namespace prefixes, reads files with more than one root element, and can remove duplicate elements.

![rust](https://img.shields.io/badge/rust-2021-000000?style=flat-square&logo=rust&logoColor=white)
![license](https://img.shields.io/badge/license-MIT-A31F34?style=flat-square)
![status](https://img.shields.io/badge/status-stable-22863A?style=flat-square)
[![ci](https://github.com/koprjaa/xml-combiner/actions/workflows/ci.yml/badge.svg)](https://github.com/koprjaa/xml-combiner/actions/workflows/ci.yml)

## Install

Install a current stable Rust toolchain, then run:

```bash
cargo install --path .
```

The executable is `xml-combiner`. Runtime dependencies are:

- `quick-xml`: buffered XML events and declared encoding support.
- `encoding_rs`: streaming UTF-16 conversion (quick-xml requires ASCII-compatible input), and encoding recognition.
- `libc`: the platform C runtime supplies local timestamps and daylight-saving rules matching the original logging.
- `tempfile`: safely stage output beside its destination and replace it only after successful serialization. Tests also use isolated directories.

No XML parser accesses external DTDs or entities. Entity declarations are rejected.

## Use

Parse every `.xml` file in a directory and write one combined file:

```bash
xml-combiner ./xmls -o combined.xml
```

Walk subdirectories, remove duplicate elements, and rename the wrapper:

```bash
xml-combiner ./feeds -o all-items.xml --recursive --deduplicate --root-element items --flatten
```

Enable the optional well formedness check:

```bash
xml-combiner ./invoices -o merged.xml --validate-schema ./invoice.xsd
```

This switch does **not** perform XSD or DTD schema validation. As in the original implementation, an existing schema path enables a parsing gate; a missing path emits a warning and processing continues. The schema contents are not read.

Raise the retry count for a slow network share:

```bash
xml-combiner /mnt/slow-nas -o out.xml --max-retries 5 --verbose
```

By default each input keeps its root element:

```xml
<combined><root>...file1...</root><root>...file2...</root></combined>
```

With `--flatten` the tool takes only the direct children of each root. Root text is discarded; child text and tails remain:

```xml
<combined>...file1 children...file2 children...</combined>
```

## Options

| Flag | Default | Effect |
|---|---|---|
| `-h, --help` | | Show help and exit. |
| `-o, --output` | `combined.xml` | Output file path. |
| `-r, --root-element` | `combined` | Name of the wrapper element. |
| `-v, --verbose` | off | Enable DEBUG logging. |
| `--recursive` | off | Walk subdirectories. |
| `--validate-schema <path>` | none | Enable the well formedness gate when this path exists. |
| `--deduplicate` | off | Skip elements with identical content. |
| `--flatten` | off | Drop the root element of each input. |
| `--max-retries <n>` | 3 | Maximum attempts per input file, including the first attempt. |

Zero or negative retry counts process no files. Paths can be absolute. Relative output paths must resolve inside the working directory, including through symlinks. Parent directories are created automatically. File discovery is sorted by path and accepts `.xml` case insensitively. Recursive traversal does not follow directory symlinks. An existing output in the input directory is included in discovery, matching the original behavior.

Exit codes: `0` when at least one input succeeds and saving succeeds, `1` for runtime failure, `2` for invalid command line arguments. Individual malformed files do not stop other inputs. An empty valid root still counts as a successfully processed file, even when flattening produces no children.

## How it works

The first pass checks each input and collects namespace declarations and used expanded names. Only successful files contribute to the output namespace map. The second pass serializes inputs to a buffered temporary file. Input files should remain unchanged during both passes.

Without deduplication both passes process XML events directly, keeping parser buffers and open-element metadata rather than complete trees. With deduplication, the second pass holds one candidate tree: an input root normally, or one direct child with `--flatten`. Namespace metadata grows with distinct names; deduplication additionally retains structural keys for unique candidates. The combined XML is never built in memory. A single huge root requires a complete candidate tree only when deduplication needs to compare it. UTF-16 is transcoded in bounded chunks.

The output is UTF-8, begins with `<?xml version='1.0' encoding='utf-8'?>` and a newline, uses `<tag />` for empty elements, and preserves input text whitespace and attribute order. It adds no indentation or final newline. Namespace declarations appear on the combined wrapper in prefix order. Comments, processing instructions, declarations and DOCTYPEs are discarded; CDATA becomes escaped text. Plain multi-root fragments and their inter-root tails are supported.

## Changes from the Python version

- **Fixed deduplication:** child tail text is included, so `<p><b/>one</p>` and `<p><b/>two</p>` remain distinct. Typed structural keys replace delimiter-based MD5 input, preventing false duplicates such as `<x>a|b=c</x>` and `<x b="c">a</x>`. Attribute order remains irrelevant; a candidate's own tail remains excluded.
- **Fixed default namespaces:** a default namespace receives a generated `nsN` prefix. Hoisting `xmlns="..."` onto an unnamespaced combined wrapper previously changed the wrapper's and unnamespaced descendants' namespace and could strip namespaces from attributes. Explicit prefixes otherwise retain the original conflict-resolution behavior. Numbered input prefixes such as `ns0` work instead of raising ElementTree's reserved-prefix exception; generated prefixes avoid collisions.
- **Fixed invalid wrapper names:** empty names, malformed XML names and unbound prefixed wrapper names are rejected with exit code `1`, rather than potentially writing invalid XML. Plain names, bound prefixed names and ElementTree-style `{uri}local` wrapper names are accepted.
- **Fixed failure isolation:** forbidden entities, invalid namespace bindings and unknown encodings fail the affected input and allow other inputs to continue, instead of escaping as uncaught exceptions. External resources are never loaded.
- **Fixed UTF-16 fragments:** BOM-marked UTF-16 multi-root files work; the previous wrapper fallback incorrectly tried decoding them as UTF-8. Invalid encoded data is rejected rather than silently replaced during fallback. ASCII, ISO-8859-1, ISO-8859-9 and ISO-8859-11 receive XML-compatible handling rather than WHATWG alias substitutions. Other legacy aliases follow `encoding_rs` mappings; Python-only codecs outside that encoding set are unsupported.
- **Streaming and output replacement:** files are read twice, and must stay unchanged during processing. Successful output replaces the destination via a sibling temporary file. A serialization/read failure during the second pass leaves the old destination intact. Replacing a file changes its inode and hard-link behavior; replacement requires write permission in the destination directory. Unix file permissions are retained for existing files, and new files use the process umask.
- **Diagnostics:** normal messages and levels retain the original wording and logger name. Parser and OS error details come from Rust, without Python tracebacks. Validation uses the same parsing pass instead of parsing again, so multi-root warnings occur once per file; verbose validation/deduplication messages can appear in a different order. CLI usage names the installed executable and help wrapping is fixed rather than terminal-dependent. Argument-error rendering can differ from argparse. Retry counts use Rust decimal syntax and signed 64-bit integers instead of Python integer syntax and arbitrary precision; command line arguments must be Unicode.
- **Fixed DTD fragment handling:** internal ATTLIST defaults and attribute type normalization also apply to multi-root fragments. The Python wrapper fallback silently discarded the DOCTYPE and lost these defaults. Single-root defaults retain their original behavior; external DTDs remain unread and no schema validation has been added.
- The Python module API is replaced by a Rust library (`Options`, `Report`, `combine`, `parse_roots`, `element_key`, `declared_encoding`, `get_xml_files`, `safe_output`). Namespace registrations are local to each run, so previous runs cannot affect later serializations.

## Development

```bash
cargo build --release
cargo fmt --check
cargo clippy --all-targets -- -D warnings
cargo test
```

Tests include a counterpart for every original Python test and parameter case, exact serialized bytes, CLI integration, namespace and deduplication regressions, UTF-16 fragments, output safety and large flattened inputs. CI runs formatting, clippy and tests on Ubuntu and Windows.

## License

[MIT](LICENSE)
