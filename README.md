# XML Combiner

## 1. Overview

A command-line utility that merges multiple XML files from a directory into a single XML file. The tool processes all XML files in a given directory and combines their root-level elements under a configurable root element.

## 2. Motivation

This project addresses the need to consolidate XML data from multiple sources into a single file for downstream processing. Common use cases include batch data processing, data migration, and report aggregation where XML files are generated separately but need to be combined for analysis.

## 3. What This Project Does

The tool scans a specified directory for XML files (optionally recursively), parses each file with namespace awareness, and combines them into a single XML structure. By default, the full structure of each input file is preserved, including root elements. The output is written to a single XML file with UTF-8 encoding and XML declaration.

The tool supports schema validation, duplicate detection, error recovery with retries, and can handle files with multiple root elements. Processing continues even if individual files fail to parse, with errors logged for review. The tool reports how many files were successfully processed.

## 4. Architecture

The project follows a separation of concerns pattern:

- **main.py**: Command-line interface layer. Handles argument parsing, logging configuration, and application entry point. Returns appropriate exit codes for shell scripting.

- **xml_combiner.py**: Core business logic layer. Contains the `XMLCombiner` class that encapsulates file discovery, validation, parsing, and combination logic.

The architecture is intentionally minimal. No dependency injection framework or complex abstractions are used, as the problem domain does not require them.

## 5. Tech Stack

- Python 3.6+ (uses standard library only)
- xml.etree.ElementTree for XML parsing and generation
- xml.sax for advanced parsing and multiple root element detection
- argparse for command-line interface
- logging for structured output
- pathlib for path handling
- hashlib for duplicate detection

No external dependencies are required. This reduces deployment complexity and avoids dependency management overhead.

## 6. Data Sources

Input: XML files located in a user-specified directory. Files are identified by `.xml` extension (case-insensitive). The tool can optionally recursively traverse subdirectories when the `--recursive` flag is used.

Output: A single XML file written to a user-specified location. The output directory is created if it does not exist. All namespaces from input files are preserved and registered in the output.

## 7. Key Design Decisions

**Standard library only**: Uses `xml.etree.ElementTree` and `xml.sax` instead of `lxml` to avoid external dependencies. This trades some performance and features for simplicity and portability.

**Class-based design**: The `XMLCombiner` class encapsulates state and behavior, making the code testable and reusable as a library component, not just a script.

**Error resilience with retries**: Individual file parsing errors trigger retry attempts (configurable, default 3). This allows recovery from transient issues while still reporting persistent failures.

**Namespace preservation**: All XML namespaces are detected, registered, and preserved in the output. This prevents namespace conflicts and maintains semantic correctness.

**Structure preservation by default**: Full XML structure is preserved including root elements. Legacy flattening behavior is available via `--flatten` flag for backward compatibility.

**Duplicate detection**: Optional content-based duplicate detection using MD5 hashing. When enabled, identical elements are skipped to prevent redundancy.

**Multiple root element support**: Files with multiple root elements (technically invalid but sometimes encountered) are detected using SAX parsing and wrapped appropriately.

**Path validation upfront**: Validates input directory existence before processing any files. Fails fast to avoid wasted computation.

**Logging over print**: Uses Python's logging module for structured, configurable output. Supports different verbosity levels for debugging.

**Sorted file processing**: Files are processed in sorted order for deterministic output, which aids reproducibility and testing.

## 8. Limitations

**Basic schema validation**: Schema validation is implemented but limited to well-formedness checking when using standard library. Full XSD/DTD validation would require external libraries like `lxml`.

**Memory-based processing**: XML files are loaded into memory for processing. Very large files (several GB) may cause memory issues. Streaming processing for individual large files is partially implemented but not fully optimized.

**Multiple root element handling**: Files with multiple root elements are detected and wrapped, but the wrapping approach is a workaround. True multi-root XML files are technically invalid per XML specification.

**Duplicate detection performance**: Content-based duplicate detection using MD5 hashing works well but may be slow for very large elements or many files. The hash computation processes the entire element tree.

**Namespace prefix conflicts**: While namespaces are preserved, if two input files use different prefixes for the same namespace URI, the output may use different prefixes. The namespace URIs are preserved correctly.

**Schema validation scope**: When `--validate-schema` is provided, the tool validates well-formedness. Full schema validation against XSD/DTD requires the schema file to be accessible, but comprehensive validation (checking all constraints) is limited by standard library capabilities.

## 9. How to Run

Prerequisites: Python 3.6 or higher.

```bash
python main.py <input_folder> [options]
```

Required argument:
- `input_folder`: Path to directory containing XML files

Optional arguments:
- `-o, --output`: Output file path (default: `combined.xml`)
- `-r, --root-element`: Name of root element in output (default: `combined`)
- `-v, --verbose`: Enable DEBUG-level logging
- `--recursive`: Recursively process XML files in subdirectories
- `--validate-schema PATH`: Path to XSD or DTD schema file for validation
- `--deduplicate`: Detect and skip duplicate elements based on content
- `--flatten`: Flatten structure (legacy behavior: only extract direct children of root)
- `--max-retries N`: Maximum retry attempts for failed file processing (default: 3)

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

Recursively process XML files in subdirectories:
```bash
python main.py /path/to/xml/files --recursive -o combined.xml
```

Process with duplicate detection and schema validation:
```bash
python main.py /path/to/xml/files --deduplicate --validate-schema schema.xsd -o combined.xml
```

Use legacy flattening behavior with error retries:
```bash
python main.py /path/to/xml/files --flatten --max-retries 5 -o combined.xml
```

## 11. Future Improvements

**Full streaming implementation**: Optimize streaming processing for very large files to reduce memory footprint further.

**Advanced schema validation**: Integrate with `lxml` or similar library for comprehensive XSD/DTD validation when external dependencies are acceptable.

**Progress reporting**: Add progress bar or percentage completion for long-running operations.

**Configuration file**: Support YAML/JSON configuration for complex scenarios instead of command-line arguments only.

**Parallel processing**: Process multiple files concurrently for performance improvement on large directories.

**Selective deduplication**: Allow users to specify which attributes or content fields to use for duplicate detection instead of full content hashing.

**Namespace prefix normalization**: Option to normalize namespace prefixes across input files for consistent output.

**Output format options**: Support for pretty-printing, compression, or different XML output formats.

## 12. Author

Jan Alexandr Kopřiva  
jan.alexandr.kopriva@gmail.com

## 13. License

MIT License

Copyright (c) 2024 Jan Alexandr Kopřiva

See LICENSE file for full license text.
