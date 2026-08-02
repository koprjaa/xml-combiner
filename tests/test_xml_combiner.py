"""Tests for the xml-combiner core module."""

import xml.etree.ElementTree as ET
from pathlib import Path

import pytest

from xml_combiner import XMLCombiner, declared_encoding, element_hash, parse_roots

SINGLE_ROOT = '<?xml version="1.0"?><root><item id="1">A</item></root>'
NAMESPACED = (
    '<?xml version="1.0"?>'
    '<feed xmlns:dc="http://purl.org/dc/elements/1.1/"><dc:title>T</dc:title></feed>'
)
MULTI_ROOT = "<one>first</one>\n<two>second</two>"


def write(folder: Path, name: str, text: str, encoding: str = "utf-8") -> Path:
    path = folder / name
    path.write_bytes(text.encode(encoding))
    return path


def combine(tmp_path: Path, files: dict[str, str], **kwargs) -> ET.Element:
    """Run a full combine over the given files and return the parsed output root."""
    source = tmp_path / "in"
    source.mkdir(exist_ok=True)
    for name, text in files.items():
        write(source, name, text)
    out = tmp_path / "out.xml"
    combiner = XMLCombiner(str(source), str(out), **kwargs)
    assert combiner.run() is True
    return ET.parse(out).getroot()


# --- element_hash -----------------------------------------------------------


def test_element_hash_is_stable_for_equal_content():
    a = ET.fromstring("<x a='1'>t</x>")
    b = ET.fromstring("<x a='1'>t</x>")
    assert element_hash(a) == element_hash(b)


def test_element_hash_separates_attribute_order_from_value():
    same = ET.fromstring("<x b='2' a='1'/>")
    reordered = ET.fromstring("<x a='1' b='2'/>")
    changed = ET.fromstring("<x a='1' b='3'/>")
    assert element_hash(same) == element_hash(reordered)
    assert element_hash(same) != element_hash(changed)


def test_element_hash_covers_text_and_children():
    base = ET.fromstring("<x><c>1</c></x>")
    other_text = ET.fromstring("<x><c>2</c></x>")
    extra_child = ET.fromstring("<x><c>1</c><c>1</c></x>")
    assert element_hash(base) != element_hash(other_text)
    assert element_hash(base) != element_hash(extra_child)


# --- declared_encoding ------------------------------------------------------


@pytest.mark.parametrize(
    ("raw", "expected"),
    [
        (b'<?xml version="1.0" encoding="windows-1250"?><a/>', "windows-1250"),
        (b"<?xml version='1.0' encoding='ISO-8859-2'?><a/>", "ISO-8859-2"),
        (b'<?xml version="1.0"?><a/>', "utf-8"),
        (b"<a/>", "utf-8"),
    ],
)
def test_declared_encoding(raw, expected):
    assert declared_encoding(raw) == expected


# --- parse_roots ------------------------------------------------------------


def test_parse_roots_single_document(tmp_path):
    roots, _ = parse_roots(write(tmp_path, "a.xml", SINGLE_ROOT))
    assert [r.tag for r in roots] == ["root"]


def test_parse_roots_keeps_every_root_of_a_multi_root_file(tmp_path):
    roots, _ = parse_roots(write(tmp_path, "m.xml", MULTI_ROOT))
    assert [r.tag for r in roots] == ["one", "two"]


def test_parse_roots_collects_namespace_prefixes(tmp_path):
    _, prefixes = parse_roots(write(tmp_path, "ns.xml", NAMESPACED))
    assert prefixes == {"dc": "http://purl.org/dc/elements/1.1/"}


def test_parse_roots_collects_prefixes_from_multi_root_files(tmp_path):
    text = '<one xmlns:dc="http://purl.org/dc/elements/1.1/"><dc:t>A</dc:t></one>\n<two/>'
    roots, prefixes = parse_roots(write(tmp_path, "mn.xml", text))
    assert len(roots) == 2
    assert prefixes == {"dc": "http://purl.org/dc/elements/1.1/"}


def test_parse_roots_honors_the_declared_encoding_on_the_wrapper_path(tmp_path):
    text = '<?xml version="1.0" encoding="windows-1250"?><a>Kopřiva</a>\n<b/>'
    roots, _ = parse_roots(write(tmp_path, "cz.xml", text, encoding="windows-1250"))
    assert [r.tag for r in roots] == ["a", "b"]
    assert roots[0].text == "Kopřiva"


def test_parse_roots_strips_a_doctype_before_wrapping(tmp_path):
    text = '<?xml version="1.0"?><!DOCTYPE note SYSTEM "note.dtd">\n<one/>\n<two/>'
    roots, _ = parse_roots(write(tmp_path, "dt.xml", text))
    assert [r.tag for r in roots] == ["one", "two"]


def test_parse_roots_raises_on_a_genuinely_malformed_document(tmp_path):
    with pytest.raises(ET.ParseError):
        parse_roots(write(tmp_path, "bad.xml", "<root><unclosed></root>"))


# --- file discovery ---------------------------------------------------------


def test_get_xml_files_ignores_other_extensions_and_is_case_insensitive(tmp_path):
    source = tmp_path / "in"
    source.mkdir()
    write(source, "a.xml", SINGLE_ROOT)
    write(source, "b.XML", SINGLE_ROOT)
    write(source, "notes.txt", "nope")
    found = XMLCombiner(str(source), str(tmp_path / "o.xml")).get_xml_files()
    assert [p.name for p in found] == ["a.xml", "b.XML"]


def test_get_xml_files_walks_subdirectories_only_when_recursive(tmp_path):
    source = tmp_path / "in"
    (source / "nested").mkdir(parents=True)
    write(source, "top.xml", SINGLE_ROOT)
    write(source / "nested", "deep.xml", SINGLE_ROOT)

    flat = XMLCombiner(str(source), str(tmp_path / "o.xml")).get_xml_files()
    deep = XMLCombiner(str(source), str(tmp_path / "o.xml"), recursive=True).get_xml_files()
    assert [p.name for p in flat] == ["top.xml"]
    assert sorted(p.name for p in deep) == ["deep.xml", "top.xml"]


# --- combining --------------------------------------------------------------


def test_combine_preserves_each_root_by_default(tmp_path):
    root = combine(tmp_path, {"a.xml": SINGLE_ROOT, "b.xml": SINGLE_ROOT})
    assert [c.tag for c in root] == ["root", "root"]


def test_flatten_lifts_the_children_out_of_each_root(tmp_path):
    root = combine(tmp_path, {"a.xml": SINGLE_ROOT}, preserve_structure=False)
    assert [c.tag for c in root] == ["item"]


def test_deduplicate_drops_identical_elements_across_files(tmp_path):
    root = combine(tmp_path, {"a.xml": SINGLE_ROOT, "b.xml": SINGLE_ROOT}, deduplicate=True)
    assert [c.tag for c in root] == ["root"]


def test_multi_root_files_reach_the_output(tmp_path):
    root = combine(tmp_path, {"m.xml": MULTI_ROOT})
    assert [c.tag for c in root] == ["one", "two"]


def test_namespace_prefix_survives_the_round_trip(tmp_path):
    source = tmp_path / "in"
    source.mkdir()
    write(source, "ns.xml", NAMESPACED)
    out = tmp_path / "out.xml"
    assert XMLCombiner(str(source), str(out)).run() is True
    text = out.read_text(encoding="utf-8")
    assert 'xmlns:dc="http://purl.org/dc/elements/1.1/"' in text
    assert "ns0:" not in text


def test_a_malformed_file_fails_without_stopping_the_others(tmp_path):
    source = tmp_path / "in"
    source.mkdir()
    write(source, "good.xml", SINGLE_ROOT)
    write(source, "bad.xml", "<root><unclosed></root>")
    combiner = XMLCombiner(str(source), str(tmp_path / "out.xml"))
    assert combiner.run() is True
    assert combiner.processed_files == 1
    assert combiner.failed_files == 1


def test_a_malformed_file_is_parsed_once_and_not_retried(tmp_path, monkeypatch):
    """A parse error is deterministic, so repeating it only wastes time."""
    import xml_combiner

    calls = []
    real = xml_combiner.parse_roots

    def counting(path):
        calls.append(path)
        return real(path)

    monkeypatch.setattr(xml_combiner, "parse_roots", counting)

    source = tmp_path / "in"
    source.mkdir()
    write(source, "bad.xml", "<root><unclosed></root>")
    XMLCombiner(str(source), str(tmp_path / "out.xml"), max_retries=3).run()
    assert len(calls) == 1


def test_empty_input_folder_reports_failure(tmp_path):
    source = tmp_path / "in"
    source.mkdir()
    assert XMLCombiner(str(source), str(tmp_path / "out.xml")).run() is False


def test_missing_input_folder_reports_failure(tmp_path):
    assert XMLCombiner(str(tmp_path / "nope"), str(tmp_path / "out.xml")).run() is False


# --- output safety ----------------------------------------------------------


def test_relative_output_may_not_escape_the_working_directory(tmp_path, monkeypatch):
    source = tmp_path / "in"
    source.mkdir()
    write(source, "a.xml", SINGLE_ROOT)
    work = tmp_path / "work"
    work.mkdir()
    monkeypatch.chdir(work)

    combiner = XMLCombiner(str(source), "../escaped.xml")
    assert combiner.run() is False
    assert not (tmp_path / "escaped.xml").exists()


def test_absolute_output_outside_the_working_directory_is_allowed(tmp_path, monkeypatch):
    source = tmp_path / "in"
    source.mkdir()
    write(source, "a.xml", SINGLE_ROOT)
    work = tmp_path / "work"
    work.mkdir()
    monkeypatch.chdir(work)

    target = tmp_path / "explicit.xml"
    assert XMLCombiner(str(source), str(target)).run() is True
    assert target.exists()
