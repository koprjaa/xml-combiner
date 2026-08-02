# xml-combiner

Merges a directory of XML files into one file. It keeps namespaces, detects files with more than one root element, and can remove duplicate elements.

![python](https://img.shields.io/badge/python-3.8+-3776AB?style=flat-square&logo=python&logoColor=white)
![license](https://img.shields.io/badge/license-MIT-A31F34?style=flat-square)
![status](https://img.shields.io/badge/status-stable-22863A?style=flat-square)

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

- Every `xmlns:*` declaration found in the inputs goes to `ET.register_namespace()`, so prefixes survive the round trip.
- A SAX pre-pass finds files with more than one root element. Plain ElementTree rejects those files.
- `--deduplicate` hashes the element tree, which covers tag, attributes, and children, then skips repeats across files.
- Each file gets up to three attempts. The run continues with the other inputs. The last log line reports how many files of the total were processed.

The tool uses `xml.etree.ElementTree` and `xml.sax` instead of `lxml`. `lxml` gives faster XPath and full XSD validation, but it needs a C compile step that breaks on fresh Windows machines, restricted CI runners, and minimal containers. For hundreds of moderate XML files the standard library parser is fast enough and installs everywhere.

## Limits

Schema validation is limited to what `xml.etree` supports. Full XSD validation needs `lxml`. The tool holds the combined tree in memory, so the output size is bound by RAM.

## License

[MIT](LICENSE)
