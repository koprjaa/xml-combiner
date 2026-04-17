# xml-combiner

**Merge a directory of XML files into one, with namespaces preserved, duplicates optional, and zero external dependencies.**

![python](https://img.shields.io/badge/python-3.6+-3776AB?style=flat-square&logo=python&logoColor=white)
![license](https://img.shields.io/badge/license-MIT-A31F34?style=flat-square)
![status](https://img.shields.io/badge/status-stable-22863A?style=flat-square)
![stdlib-only](https://img.shields.io/badge/deps-stdlib%20only-555?style=flat-square)

Stdlib-only. No `pip install` surprises, no `lxml` compile nightmares. Just `python main.py <dir>` and you have one combined file.

## Quick start

```bash
python main.py ./xmls -o combined.xml
```

That's it for the default case — parse every `.xml` in `./xmls`, wrap their roots under `<combined>`, write to `combined.xml`.

## Cookbook

Flatten a directory of RSS-style feeds, de-dup items, walk subdirs:

```bash
python main.py ./feeds -o all-items.xml \
  --recursive --deduplicate --root-element items --flatten
```

Validate against a schema before combining (fail the run if any file doesn't match):

```bash
python main.py ./invoices -o merged.xml --validate-schema ./invoice.xsd
```

Aggressive retries for a flaky network-mounted directory:

```bash
python main.py /mnt/slow-nas -o out.xml --max-retries 5 --verbose
```

Keep each file's full root structure (default — nothing is dropped):

```bash
python main.py ./data -o combined.xml
# → <combined><root>...file1...</root><root>...file2...</root></combined>
```

vs. `--flatten`, which extracts only the direct children of each root:

```bash
python main.py ./data -o combined.xml --flatten
# → <combined>...file1 children...file2 children...</combined>
```

## Features worth knowing about

- **Namespace preservation.** Every `xmlns:*` declaration found in the inputs is registered with `ET.register_namespace()` so prefixes survive the round-trip.
- **Multi-root detection.** A SAX pre-pass catches files with more than one root element (which plain ElementTree would reject) and handles them gracefully.
- **Deduplication.** `--deduplicate` hashes the full element tree (tag + attributes + recursive children) with MD5 and skips repeats across files.
- **Per-file retry loop.** Default 3 attempts before giving up on a file; the run continues with the remaining inputs. Final log line reports `N of M processed`.
- **Structured logging.** `INFO` for file counts, `DEBUG` (`-v`) for per-file tracing, `WARNING` for parse errors and duplicate roots.

## All flags

```
usage: main.py [-h] [-o OUTPUT] [-r ROOT_ELEMENT] [-v] [--recursive]
               [--validate-schema VALIDATE_SCHEMA] [--deduplicate] [--flatten]
               [--max-retries MAX_RETRIES]
               input_folder
```

| flag | default | effect |
|------|---------|--------|
| `-o, --output` | `combined.xml` | output file path |
| `-r, --root-element` | `combined` | wrapper element name |
| `-v, --verbose` | off | enable DEBUG logging |
| `--recursive` | off | walk subdirectories |
| `--validate-schema <path>` | — | XSD/DTD to validate against |
| `--deduplicate` | off | hash + skip identical elements |
| `--flatten` | off | drop root element wrappers |
| `--max-retries <n>` | 3 | per-file retry attempts |

## Why stdlib only

Using `lxml` would buy faster XPath and full XSD validation, but it also drags in C compile steps that break on fresh Windows environments, locked-down CI runners, and minimal containers. For the 80% case — combining hundreds of modestly-sized XML files — `xml.etree.ElementTree` + `xml.sax` is fast enough and installs instantly everywhere.

## License

[MIT](LICENSE)
