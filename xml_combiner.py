"""
Project: xml-combiner
File: xml_combiner.py
Description: Core module for combining multiple XML files into a single XML file.
Author: Jan Alexandr Kopřiva jan.alexandr.kopriva@gmail.com
License: MIT
"""

import hashlib
import io
import logging
import re
import xml.etree.ElementTree as ET
from pathlib import Path
from xml.etree.ElementTree import Element

# defusedxml defends against entity-expansion ("billion laughs") and external-entity
# DoS when parsing untrusted input. Drop-in replacements for the stdlib parsers.
from defusedxml.ElementTree import fromstring as defused_fromstring
from defusedxml.ElementTree import iterparse as defused_iterparse
from defusedxml.ElementTree import parse as defused_parse

logger = logging.getLogger(__name__)

# Matches the XML declaration and a DOCTYPE at the start of a document. Both are
# legal only at the top level, so they must come off before the text is wrapped.
_PROLOG_RE = re.compile(r"\A\s*(?:<\?xml[^>]*\?>\s*)?(?:<!DOCTYPE[^>\[]*(?:\[[^\]]*\])?>\s*)?")
_ENCODING_RE = re.compile(rb"\A\s*<\?xml[^>]*?encoding\s*=\s*[\"']([\w.-]+)[\"']")

WRAPPER_TAG = "__xml_combiner_wrapper__"


def element_hash(element: Element) -> str:
    """Content hash of an element, covering tag, text, attributes and children."""

    def serialize(elem: Element) -> str:
        parts = [f"{elem.tag}:{elem.text or ''}"]
        parts.extend(f"{key}={value}" for key, value in sorted(elem.attrib.items()))
        parts.extend(serialize(child) for child in elem)
        return "|".join(parts)

    # Not a security boundary, only duplicate detection.
    return hashlib.md5(serialize(element).encode("utf-8"), usedforsecurity=False).hexdigest()


def declared_encoding(raw: bytes) -> str:
    """Encoding named in the XML declaration, or utf-8 when it does not say."""
    match = _ENCODING_RE.match(raw)
    return match.group(1).decode("ascii") if match else "utf-8"


def parse_roots(xml_file: Path) -> tuple[list[Element], dict[str, str]]:
    """Read one XML file and return its root elements and namespace prefixes.

    A well-formed document has exactly one root. Files with several top-level
    elements are not valid XML and ElementTree refuses them outright, so they are
    re-read and parsed inside a synthetic wrapper element. Both cases come back
    as a list, which lets the caller treat them the same way.
    """
    try:
        roots = [defused_parse(xml_file).getroot()]
        prefixes = _namespace_prefixes(xml_file)
    except ET.ParseError:
        raw = xml_file.read_bytes()
        body = _PROLOG_RE.sub("", raw.decode(declared_encoding(raw), errors="replace"))
        wrapped = f"<{WRAPPER_TAG}>{body}</{WRAPPER_TAG}>"
        roots = list(defused_fromstring(wrapped))
        if len(roots) > 1:
            logger.warning("%s has %d root elements, keeping all of them", xml_file.name, len(roots))
        prefixes = _namespace_prefixes(io.BytesIO(wrapped.encode("utf-8")))
    return roots, prefixes


def _namespace_prefixes(source) -> dict[str, str]:
    """Prefix to URI map from a document. iterparse yields (event, data) pairs."""
    return {prefix: uri for _, (prefix, uri) in defused_iterparse(source, events=("start-ns",))}


class XMLCombiner:
    """Combines XML files from a directory into a single XML file."""

    def __init__(
        self,
        input_folder: str,
        output_file: str,
        root_element_name: str = "combined",
        recursive: bool = False,
        validate_schema: str | None = None,
        deduplicate: bool = False,
        preserve_structure: bool = True,
        max_retries: int = 3,
    ):
        self.input_folder = Path(input_folder)
        self.output_file = Path(output_file)
        self.root_element_name = root_element_name
        self.recursive = recursive
        self.validate_schema = validate_schema
        self.deduplicate = deduplicate
        self.preserve_structure = preserve_structure
        self.max_retries = max_retries

        self.combined_root = ET.Element(root_element_name)
        self.seen_elements: set[str] = set()
        self.namespace_map: dict[str, str] = {}
        self.processed_files = 0
        self.failed_files = 0

    def validate_paths(self) -> bool:
        if not self.input_folder.exists():
            logger.error("Input folder does not exist: %s", self.input_folder)
            return False
        if not self.input_folder.is_dir():
            logger.error("Path is not a directory: %s", self.input_folder)
            return False
        return True

    def get_xml_files(self) -> list[Path]:
        """XML files in the input folder, optionally including subdirectories."""
        entries = self.input_folder.rglob("*") if self.recursive else self.input_folder.iterdir()
        xml_files = sorted(p for p in entries if p.is_file() and p.suffix.lower() == ".xml")
        logger.info("Found %d XML files", len(xml_files))
        return xml_files

    def _register_prefixes(self, prefixes: dict[str, str]) -> None:
        """Keep the prefix names the inputs used, instead of ET's ns0, ns1, ns2."""
        for prefix, uri in prefixes.items():
            if uri not in self.namespace_map.values():
                self.namespace_map[prefix or ""] = uri
                ET.register_namespace(prefix or "", uri)

    def _is_new(self, element: Element) -> bool:
        """False when deduplication is on and this element was already added."""
        if not self.deduplicate:
            return True
        digest = element_hash(element)
        if digest in self.seen_elements:
            logger.debug("Skipping duplicate element: %s", element.tag)
            return False
        self.seen_elements.add(digest)
        return True

    def _add_root(self, root: Element) -> None:
        """Append one parsed root, honoring preserve_structure and deduplication."""
        candidates = [root] if self.preserve_structure else list(root)
        for element in candidates:
            if self._is_new(element):
                self.combined_root.append(element)

    def _validate_xml(self, xml_file: Path) -> bool:
        """Well-formedness gate for --validate-schema.

        Full XSD validation needs lxml, which this project deliberately avoids.
        The schema path is therefore only a switch that turns the check on.
        """
        if not self.validate_schema:
            return True
        if not Path(self.validate_schema).exists():
            logger.warning("Schema file not found, skipping validation: %s", self.validate_schema)
            return True
        try:
            parse_roots(xml_file)
        except ET.ParseError:
            logger.exception("Validation failed for %s", xml_file.name)
            return False
        else:
            logger.debug("Validated %s", xml_file.name)
            return True

    def _process_xml_file(self, xml_file: Path) -> bool:
        """Parse one file into the combined tree.

        Only OSError is retried. A read over a network share can fail once and
        succeed on the next attempt, but a malformed document parses the same way
        every time, so retrying a ParseError only repeats the same failure.
        """
        if not self._validate_xml(xml_file):
            return False

        for attempt in range(1, self.max_retries + 1):
            try:
                roots, prefixes = parse_roots(xml_file)
            except OSError as exc:
                if attempt < self.max_retries:
                    logger.warning(
                        "Read error (attempt %d/%d) on %s: %s",
                        attempt, self.max_retries, xml_file.name, exc,
                    )
                    continue
                logger.exception("Cannot read %s after %d attempts", xml_file.name, self.max_retries)
                return False
            except ET.ParseError:
                logger.exception("Malformed XML in %s", xml_file.name)
                return False

            self._register_prefixes(prefixes)
            for root in roots:
                self._add_root(root)
            return True

        return False

    def combine_xml_files(self) -> bool:
        if not self.validate_paths():
            return False

        xml_files = self.get_xml_files()
        if not xml_files:
            logger.warning("No XML files found to process")
            return False

        self.processed_files = 0
        self.failed_files = 0

        for xml_file in xml_files:
            if self._process_xml_file(xml_file):
                self.processed_files += 1
                logger.debug("Processed file: %s", xml_file.name)
            else:
                self.failed_files += 1

        logger.info("Successfully processed %d of %d files", self.processed_files, len(xml_files))
        if self.failed_files:
            logger.warning("Failed to process %d files", self.failed_files)

        return self.processed_files > 0

    def _resolve_safe_output(self) -> Path | None:
        """Resolve the output path, rejecting relative-path traversal.

        Relative output paths must stay under the current working directory
        (the intended base). Absolute paths are treated as an explicit,
        deliberate operator choice and are allowed as-is. This blocks
        ``../../etc/passwd``-style traversal via a relative ``--output``.
        """
        base = Path.cwd().resolve()
        try:
            resolved = self.output_file.resolve()
        except OSError:
            logger.exception("Invalid output path %s", self.output_file)
            return None

        if not self.output_file.is_absolute() and not (resolved == base or base in resolved.parents):
            logger.error(
                "Refusing to write outside working directory: %s -> %s",
                self.output_file, resolved,
            )
            return None
        return resolved

    def save_combined_xml(self) -> bool:
        safe_output = self._resolve_safe_output()
        if safe_output is None:
            return False
        self.output_file = safe_output

        try:
            self.output_file.parent.mkdir(parents=True, exist_ok=True)
            for prefix, uri in self.namespace_map.items():
                ET.register_namespace(prefix, uri)
            ET.ElementTree(self.combined_root).write(
                self.output_file, encoding="utf-8", xml_declaration=True, method="xml"
            )
        except OSError:
            logger.exception("Error saving file %s", self.output_file)
            return False
        else:
            logger.info("Combined XML file saved: %s", self.output_file)
            return True

    def run(self) -> bool:
        return self.combine_xml_files() and self.save_combined_xml()
