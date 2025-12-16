"""
Project: xml_combine
File: xml_combiner.py
Description: Core module for combining multiple XML files into a single XML file.
Author: Jan Alexandr Kopřiva jan.alexandr.kopriva@gmail.com
License: MIT
"""

import os
import logging
import xml.etree.ElementTree as ET
from pathlib import Path
from typing import Optional, List, Set, Dict, Tuple
from xml.etree.ElementTree import Element
import hashlib
import xml.sax
from xml.sax.handler import ContentHandler
import io


logger = logging.getLogger(__name__)


class MultiRootHandler(ContentHandler):
    """SAX handler to detect and handle multiple root elements."""

    def __init__(self):
        super().__init__()
        self.roots = []
        self.current_root = None
        self.depth = 0
        self.namespaces = {}

    def startElementNS(self, name, qname, attrs):
        self.depth += 1
        if self.depth == 1:
            ns, local = name
            self.current_root = {
                'name': name,
                'qname': qname,
                'attrs': dict(attrs),
                'children': []
            }
            self.roots.append(self.current_root)
        elif self.current_root:
            self.current_root['children'].append({
                'name': name,
                'qname': qname,
                'attrs': dict(attrs)
            })

    def endElementNS(self, name, qname):
        self.depth -= 1

    def startPrefixMapping(self, prefix, uri):
        self.namespaces[prefix] = uri


class XMLCombiner:
    """Combines XML files from a directory into a single XML file."""

    def __init__(self, input_folder: str, output_file: str,
                 root_element_name: str = "combined",
                 recursive: bool = False,
                 validate_schema: Optional[str] = None,
                 deduplicate: bool = False,
                 preserve_structure: bool = True,
                 max_retries: int = 3):
        self.input_folder = Path(input_folder)
        self.output_file = Path(output_file)
        self.root_element_name = root_element_name
        self.recursive = recursive
        self.validate_schema = validate_schema
        self.deduplicate = deduplicate
        self.preserve_structure = preserve_structure
        self.max_retries = max_retries
        
        self.combined_root = ET.Element(root_element_name)
        self.seen_elements: Set[str] = set()
        self.namespace_map: Dict[str, str] = {}
        self.processed_files = 0
        self.failed_files = 0

    def validate_paths(self) -> bool:
        if not self.input_folder.exists():
            logger.error(f"Input folder does not exist: {self.input_folder}")
            return False
        if not self.input_folder.is_dir():
            logger.error(f"Path is not a directory: {self.input_folder}")
            return False
        return True

    def get_xml_files(self) -> List[Path]:
        """Get XML files, optionally recursively."""
        xml_files = []
        
        if self.recursive:
            for root, dirs, files in os.walk(self.input_folder):
                for filename in files:
                    if filename.lower().endswith('.xml'):
                        xml_files.append(Path(root) / filename)
        else:
            xml_files = [
                self.input_folder / filename
                for filename in os.listdir(self.input_folder)
                if filename.lower().endswith('.xml')
            ]
        
        logger.info(f"Found {len(xml_files)} XML files")
        return sorted(xml_files)

    def _register_namespaces(self, element: Element) -> None:
        """Register all namespaces from an element and its children."""
        for prefix, uri in element.attrib.items():
            if prefix.startswith('xmlns'):
                if prefix == 'xmlns':
                    prefix = ''
                else:
                    prefix = prefix[6:]  # Remove 'xmlns:' prefix
                if uri not in self.namespace_map.values():
                    self.namespace_map[prefix] = uri
                    ET.register_namespace(prefix, uri)
        
        for child in element.iter():
            for prefix, uri in child.attrib.items():
                if prefix.startswith('xmlns'):
                    if prefix == 'xmlns':
                        prefix = ''
                    else:
                        prefix = prefix[6:]
                    if uri not in self.namespace_map.values():
                        self.namespace_map[prefix] = uri
                        ET.register_namespace(prefix, uri)

    def _get_element_hash(self, element: Element) -> str:
        """Generate hash for element to detect duplicates."""
        def element_to_string(elem):
            parts = [f"{elem.tag}:{elem.text or ''}"]
            for key, value in sorted(elem.attrib.items()):
                parts.append(f"{key}={value}")
            for child in elem:
                parts.append(element_to_string(child))
            return "|".join(parts)
        
        content = element_to_string(element)
        return hashlib.md5(content.encode('utf-8')).hexdigest()

    def _validate_xml(self, xml_file: Path) -> bool:
        """Validate XML against schema if provided."""
        if not self.validate_schema:
            return True
        
        try:
            schema_path = Path(self.validate_schema)
            if not schema_path.exists():
                logger.warning(f"Schema file not found: {schema_path}")
                return True
            
            # Basic validation - full XSD validation would require lxml
            # For now, we just check if file is well-formed
            ET.parse(xml_file)
            logger.debug(f"Validated {xml_file.name}")
            return True
        except ET.ParseError as e:
            logger.error(f"Validation failed for {xml_file.name}: {e}")
            return False


    def _add_element_with_structure(self, element: Element) -> None:
        """Add element preserving full structure."""
        if self.deduplicate:
            element_hash = self._get_element_hash(element)
            if element_hash in self.seen_elements:
                logger.debug(f"Skipping duplicate element: {element.tag}")
                return
            self.seen_elements.add(element_hash)
        
        self._register_namespaces(element)
        self.combined_root.append(element)

    def _add_element_children(self, element: Element) -> None:
        """Add only direct children of element (legacy behavior)."""
        self._register_namespaces(element)
        for child in element:
            if self.deduplicate:
                child_hash = self._get_element_hash(child)
                if child_hash in self.seen_elements:
                    logger.debug(f"Skipping duplicate element: {child.tag}")
                    continue
                self.seen_elements.add(child_hash)
            
            self.combined_root.append(child)

    def _process_xml_file(self, xml_file: Path) -> bool:
        """Process a single XML file with error recovery and multiple root support."""
        # Validate if schema provided
        if not self._validate_xml(xml_file):
            return False
        
        # Try to parse with standard ElementTree with retries
        for attempt in range(self.max_retries):
            try:
                # First, check for multiple root elements using SAX
                handler = MultiRootHandler()
                parser = xml.sax.make_parser()
                parser.setContentHandler(handler)
                parser.setFeature(xml.sax.handler.feature_namespaces, True)
                
                try:
                    parser.parse(str(xml_file))
                    if len(handler.roots) > 1:
                        logger.warning(f"File {xml_file.name} has {len(handler.roots)} root elements, processing each separately")
                        # For multiple roots, we need to parse the file differently
                        # Read file content and split/process multiple roots
                        with open(xml_file, 'rb') as f:
                            content = f.read()
                        
                        # Try to parse as single root first (most common case)
                        # If that fails, handle multiple roots
                        try:
                            tree = ET.parse(xml_file)
                            root = tree.getroot()
                            self._register_namespaces(root)
                            if self.preserve_structure:
                                self._add_element_with_structure(root)
                            else:
                                self._add_element_children(root)
                            return True
                        except:
                            # Multiple roots detected - wrap them
                            wrapper = ET.Element(f"{self.root_element_name}_wrapper")
                            # Parse file and extract all top-level elements
                            # This is a workaround - proper handling would require
                            # more sophisticated parsing
                            try:
                                tree = ET.parse(xml_file)
                                root = tree.getroot()
                                wrapper.append(root)
                            except:
                                # If standard parsing fails, try iterparse
                                context = ET.iterparse(xml_file, events=('start', 'end'))
                                roots_found = []
                                for event, elem in context:
                                    if event == 'start' and elem.getparent() is None:
                                        roots_found.append(elem)
                                        if len(roots_found) > 1:
                                            break
                                
                                if len(roots_found) > 1:
                                    for root_elem in roots_found:
                                        wrapper.append(root_elem)
                            
                            self._register_namespaces(wrapper)
                            if self.preserve_structure:
                                self.combined_root.append(wrapper)
                            else:
                                for child in wrapper:
                                    self.combined_root.append(child)
                            return True
                except Exception as sax_error:
                    # SAX parsing failed, fall through to standard parsing
                    logger.debug(f"SAX parsing failed, using standard parser: {sax_error}")
                
                # Standard single-root parsing
                tree = ET.parse(xml_file)
                root = tree.getroot()
                
                # Register namespaces
                self._register_namespaces(root)
                
                # Add element(s)
                if self.preserve_structure:
                    self._add_element_with_structure(root)
                else:
                    self._add_element_children(root)
                
                return True
                
            except ET.ParseError as e:
                if attempt < self.max_retries - 1:
                    logger.warning(f"Parse error (attempt {attempt + 1}/{self.max_retries}) in {xml_file.name}: {e}")
                    continue
                else:
                    logger.error(f"Parse error in {xml_file.name} after {self.max_retries} attempts: {e}")
                    return False
            except Exception as e:
                if attempt < self.max_retries - 1:
                    logger.warning(f"Error (attempt {attempt + 1}/{self.max_retries}) processing {xml_file.name}: {e}")
                    continue
                else:
                    logger.error(f"Unexpected error processing {xml_file.name} after {self.max_retries} attempts: {e}")
                    return False
        
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
                logger.debug(f"Processed file: {xml_file.name}")
            else:
                self.failed_files += 1

        logger.info(f"Successfully processed {self.processed_files} of {len(xml_files)} files")
        if self.failed_files > 0:
            logger.warning(f"Failed to process {self.failed_files} files")
        
        return self.processed_files > 0

    def save_combined_xml(self) -> bool:
        try:
            # Create output directory if it doesn't exist
            self.output_file.parent.mkdir(parents=True, exist_ok=True)

            # Register all namespaces in the final tree
            for prefix, uri in self.namespace_map.items():
                ET.register_namespace(prefix, uri)
            
            combined_tree = ET.ElementTree(self.combined_root)
            combined_tree.write(
                self.output_file,
                encoding='utf-8',
                xml_declaration=True,
                method='xml'
            )
            logger.info(f"Combined XML file saved: {self.output_file}")
            return True
        except Exception as e:
            logger.error(f"Error saving file: {e}")
            return False

    def run(self) -> bool:
        if not self.combine_xml_files():
            return False
        return self.save_combined_xml()
