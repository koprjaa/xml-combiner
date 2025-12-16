"""
Project: xml_combine
File: xml_combiner.py
Description: Core module for combining multiple XML files into a single XML file.
Author: Jan Alexandr Kopřiva jan.alexandr.kopriva@gmail.com
License: Proprietary
"""

import os
import logging
import xml.etree.ElementTree as ET
from pathlib import Path
from typing import Optional, List


logger = logging.getLogger(__name__)


class XMLCombiner:
    """Combines XML files from a directory into a single XML file."""

    def __init__(self, input_folder: str, output_file: str,
                 root_element_name: str = "combined"):
        self.input_folder = Path(input_folder)
        self.output_file = Path(output_file)
        self.root_element_name = root_element_name
        self.combined_root = ET.Element(root_element_name)

    def validate_paths(self) -> bool:
        if not self.input_folder.exists():
            logger.error(f"Input folder does not exist: {self.input_folder}")
            return False
        if not self.input_folder.is_dir():
            logger.error(f"Path is not a directory: {self.input_folder}")
            return False
        return True

    def get_xml_files(self) -> List[Path]:
        xml_files = [
            self.input_folder / filename
            for filename in os.listdir(self.input_folder)
            if filename.lower().endswith('.xml')
        ]
        logger.info(f"Found {len(xml_files)} XML files")
        return sorted(xml_files)

    def combine_xml_files(self) -> bool:
        if not self.validate_paths():
            return False

        xml_files = self.get_xml_files()
        if not xml_files:
            logger.warning("No XML files found to process")
            return False

        processed_count = 0
        for xml_file in xml_files:
            try:
                self._process_xml_file(xml_file)
                processed_count += 1
                logger.debug(f"Processed file: {xml_file.name}")
            except ET.ParseError as e:
                logger.error(f"Parse error in {xml_file.name}: {e}")
            except Exception as e:
                logger.error(f"Unexpected error processing {xml_file.name}: {e}")

        logger.info(f"Successfully processed {processed_count} of {len(xml_files)} files")
        return processed_count > 0

    def _process_xml_file(self, xml_file: Path) -> None:
        tree = ET.parse(xml_file)
        root = tree.getroot()

        for element in root:
            self.combined_root.append(element)

    def save_combined_xml(self) -> bool:
        try:
            # Create output directory if it doesn't exist
            self.output_file.parent.mkdir(parents=True, exist_ok=True)

            combined_tree = ET.ElementTree(self.combined_root)
            combined_tree.write(
                self.output_file,
                encoding='utf-8',
                xml_declaration=True
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

