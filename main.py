"""
Project: xml_combine
File: main.py
Description: Command-line entry point for the XML combiner application.
Author: Jan Alexandr Kopřiva jan.alexandr.kopriva@gmail.com
License: MIT
"""

import argparse
import logging
import sys
from pathlib import Path

from xml_combiner import XMLCombiner


def setup_logging(verbose: bool = False) -> None:
    level = logging.DEBUG if verbose else logging.INFO
    logging.basicConfig(
        level=level,
        format='%(asctime)s - %(name)s - %(levelname)s - %(message)s',
        datefmt='%Y-%m-%d %H:%M:%S'
    )


def main() -> int:
    parser = argparse.ArgumentParser(
        description='Combines multiple XML files from a directory into a single file.'
    )
    parser.add_argument(
        'input_folder',
        type=str,
        help='Path to directory containing XML files'
    )
    parser.add_argument(
        '-o', '--output',
        type=str,
        default='combined.xml',
        help='Path to output file (default: combined.xml)'
    )
    parser.add_argument(
        '-r', '--root-element',
        type=str,
        default='combined',
        help='Root element name (default: combined)'
    )
    parser.add_argument(
        '-v', '--verbose',
        action='store_true',
        help='Enable verbose output (DEBUG level)'
    )
    parser.add_argument(
        '--recursive',
        action='store_true',
        help='Recursively process XML files in subdirectories'
    )
    parser.add_argument(
        '--validate-schema',
        type=str,
        default=None,
        help='Path to XSD or DTD schema file for validation'
    )
    parser.add_argument(
        '--deduplicate',
        action='store_true',
        help='Detect and skip duplicate elements based on content'
    )
    parser.add_argument(
        '--flatten',
        action='store_true',
        help='Flatten structure: only extract direct children of root (legacy behavior)'
    )
    parser.add_argument(
        '--max-retries',
        type=int,
        default=3,
        help='Maximum retry attempts for failed file processing (default: 3)'
    )

    args = parser.parse_args()

    setup_logging(args.verbose)

    combiner = XMLCombiner(
        input_folder=args.input_folder,
        output_file=args.output,
        root_element_name=args.root_element,
        recursive=args.recursive,
        validate_schema=args.validate_schema,
        deduplicate=args.deduplicate,
        preserve_structure=not args.flatten,
        max_retries=args.max_retries
    )

    if combiner.run():
        return 0
    return 1


if __name__ == '__main__':
    sys.exit(main())
