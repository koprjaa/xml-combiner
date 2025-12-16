# XML Combiner

## 1. Overview

A command-line utility that merges multiple XML files from a directory into a single XML file. The tool processes all XML files in a given directory and combines their root-level elements under a configurable root element.

## 2. Motivation

This project addresses the need to consolidate XML data from multiple sources into a single file for downstream processing. Common use cases include batch data processing, data migration, and report aggregation where XML files are generated separately but need to be combined for analysis.

## 3. What This Project Does

The tool scans a specified directory for XML files, parses each file, extracts all direct child elements from the root node, and appends them to a new combined XML structure. The output is written to a single XML file with UTF-8 encoding and XML declaration.

Processing continues even if individual files fail to parse, with errors logged for review. The tool reports how many files were successfully processed.

## 4. Architecture

The project follows a separation of concerns pattern:

- **main.py**: Command-line interface layer. Handles argument parsing, logging configuration, and application entry point. Returns appropriate exit codes for shell scripting.

- **xml_combiner.py**: Core business logic layer. Contains the `XMLCombiner` class that encapsulates file discovery, validation, parsing, and combination logic.

The architecture is intentionally minimal. No dependency injection framework or complex abstractions are used, as the problem domain does not require them.

## 5. Tech Stack

- Python 3.6+ (uses standard library only)
- xml.etree.ElementTree for XML parsing and generation
- argparse for command-line interface
- logging for structured output
- pathlib for path handling

No external dependencies are required. This reduces deployment complexity and avoids dependency management overhead.

## 6. Data Sources

Input: XML files located in a user-specified directory. Files are identified by `.xml` extension (case-insensitive). The tool does not recursively traverse subdirectories.

Output: A single XML file written to a user-specified location. The output directory is created if it does not exist.

## 7. Key Design Decisions

**Standard library only**: Uses `xml.etree.ElementTree` instead of `lxml` to avoid external dependencies. This trades some performance and features for simplicity and portability.

**Class-based design**: The `XMLCombiner` class encapsulates state and behavior, making the code testable and reusable as a library component, not just a script.

**Error resilience**: Individual file parsing errors do not stop the entire process. This allows partial success when some input files are malformed.

**Path validation upfront**: Validates input directory existence before processing any files. Fails fast to avoid wasted computation.

**Logging over print**: Uses Python's logging module for structured, configurable output. Supports different verbosity levels for debugging.

**Sorted file processing**: Files are processed in sorted order for deterministic output, which aids reproducibility and testing.

## 8. Limitations

**No namespace handling**: XML namespaces are not explicitly managed. Elements are copied as-is, which may cause namespace conflicts in the output.

**No schema validation**: Input files are not validated against XML schemas. Malformed XML may cause parsing errors, but well-formed invalid XML will be processed.

**Structure flattening**: Only direct children of each input file's root element are extracted. Nested structures deeper than one level are preserved, but the root element itself is discarded.

**No duplicate detection**: Duplicate elements across files are not identified or deduplicated. All elements are appended regardless of content.

**Memory-based processing**: All XML files are loaded into memory simultaneously. Large files or many files may cause memory issues. Streaming is not implemented.

**No recursive directory traversal**: Only files in the specified directory are processed. Subdirectories are ignored.

**No XML validation**: The tool does not validate XML against DTDs or XSD schemas before or after combination.

**Limited error recovery**: If a file fails to parse, processing continues but the file is skipped. No retry mechanism or partial recovery.

**Single root element assumption**: Each input file must have a single root element. XML files with multiple root elements (which are technically invalid) may cause unexpected behavior.

## 9. How to Run

Prerequisites: Python 3.6 or higher.

```bash
python main.py <input_folder> [options]
```

Required argument:
- `input_folder`: Path to directory containing XML files

Optional arguments:
- `-o, --output`: Output file path (default: `combined.xml`)
- `-r, --root-element`: Name of root element in output (default: `root`)
- `-v, --verbose`: Enable DEBUG-level logging

Exit codes:
- `0`: Success
- `1`: Failure (validation error, no files found, or save error)

## 10. Example Usage

Combine all XML files in current directory:
```bash
python main.py .
```

Specify custom output file and root element:
```bash
python main.py /path/to/xml/files -o merged_data.xml -r "data"
```

Enable verbose logging for debugging:
```bash
python main.py /path/to/xml/files -v
```

## 11. Future Improvements

**Streaming processing**: Process files one at a time to reduce memory footprint for large datasets.

**Recursive directory traversal**: Add option to process XML files in subdirectories.

**Namespace management**: Explicitly handle XML namespaces to prevent conflicts and preserve namespace declarations.

**Schema validation**: Add optional XSD/DTD validation before processing.

**Deduplication**: Option to identify and handle duplicate elements based on content or attributes.

**Progress reporting**: Add progress bar or percentage completion for long-running operations.

**Configuration file**: Support YAML/JSON configuration for complex scenarios instead of command-line arguments only.

**Parallel processing**: Process multiple files concurrently for performance improvement on large directories.

## 12. Author

Jan Alexandr Kopřiva  
jan.alexandr.kopriva@gmail.com

## 13. License

Proprietary
