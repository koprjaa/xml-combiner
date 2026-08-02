# xml-combiner

Merges a directory of XML files into one file. It keeps namespace prefixes, reads files with more than one root element, and can remove duplicate elements.

![python](https://img.shields.io/badge/python-3.10+-3776AB?style=flat-square&logo=python&logoColor=white)
![license](https://img.shields.io/badge/license-MIT-A31F34?style=flat-square)
![status](https://img.shields.io/badge/status-stable-22863A?style=flat-square)
[![ci](https://github.com/koprjaa/xml-combiner/actions/workflows/ci.yml/badge.svg)](https://github.com/koprjaa/xml-combiner/actions/workflows/ci.yml)

## Install

```bash
pip install -r requirements.txt
```

The only dependency is [defusedxml](https://pypi.org/project/defusedxml/), which blocks entity expansion and external entity attacks in untrusted XML. Everything else comes from the standard library.

## Use

Parse every `.xml` file in a directory and write one combined file:

```bash
python main.py ./xmls -o combined.xml
```

Walk subdirectories, remove duplicate elements, and rename the wrapper:

```bash
python main.py ./feeds -o all-items.xml --recursive --deduplicate --root-element items --flatten
```

Validate against a schema first. The run fails if a file does not match:

```bash
python main.py ./invoices -o merged.xml --validate-schema ./invoice.xsd
```

Raise the retry count for a slow network share:

```bash
python main.py /mnt/slow-nas -o out.xml --max-retries 5 --verbose
```

By default each input keeps its root element:

```
<combined><root>...file1...</root><root>...file2...</root></combined>
```

With `--flatten` the tool takes the direct children of each root and drops the wrapper:

```
<combined>...file1 children...file2 children...</combined>
```

## Options

| Flag | Default | Effect |
|---|---|---|
| `-o, --output` | `combined.xml` | Output file path. |
| `-r, --root-element` | `combined` | Name of the wrapper element. |
| `-v, --verbose` | off | Enable DEBUG logging. |
| `--recursive` | off | Walk subdirectories. |
| `--validate-schema <path>` | none | XSD or DTD to validate against. |
| `--deduplicate` | off | Hash elements and skip repeats. |
| `--flatten` | off | Drop the root element of each input. |
| `--max-retries <n>` | 3 | Retry attempts per file. |

## How it works

- Namespace prefixes come from the `start-ns` events of the parser and go to `ET.register_namespace()`. Without that step ElementTree writes `ns0`, `ns1`, `ns2` instead of the prefixes the inputs used.
- A file with several top-level elements is not valid XML, and ElementTree refuses it. The tool catches that error, strips the XML declaration and any DOCTYPE, wraps the remaining text in one synthetic element, and parses that. Every root then reaches the output.
- `--deduplicate` hashes the element tree, which covers tag, text, attributes, and children, then skips repeats across files.
- Only read errors are retried, up to `--max-retries` times. A read over a network share can fail once and work on the next attempt. A malformed document parses the same way every time, so it fails at once instead of repeating the same error three times.
- One bad file does not stop the run. The last log line reports how many files of the total were processed.

The tool uses `xml.etree.ElementTree` and `xml.sax` instead of `lxml`. `lxml` gives faster XPath and full XSD validation, but it needs a C compile step that breaks on fresh Windows machines, restricted CI runners, and minimal containers. For hundreds of moderate XML files the standard library parser is fast enough and installs everywhere.

## Limits

`--validate-schema` does not validate against the schema. It turns on a well formedness check and rejects files that do not parse. Full XSD validation needs `lxml`, which this project avoids on purpose. The tool holds the combined tree in memory, so the output size is bound by RAM.

## Development

```bash
uv run --extra dev ruff check .
uv run --extra dev pytest -q
```

CI runs both on Python 3.10, 3.11, and 3.12, on Linux and Windows.

## License

[MIT](LICENSE)
