// Project: xml-combiner
// File: tests/core.rs
// Description: Equivalents of the original tests and regression coverage.
// Author: Jan Alexandr Kopřiva
// License: MIT

use std::{
    fs,
    path::{Path, PathBuf},
};
use tempfile::TempDir;
use xml_combiner::{
    combine, declared_encoding, element_key, get_xml_files, parse_roots, Options, Report,
};
const SINGLE: &str = "<?xml version=\"1.0\"?><root><item id=\"1\">A</item></root>";
const NS: &str = "<?xml version=\"1.0\"?><feed xmlns:dc=\"http://purl.org/dc/elements/1.1/\"><dc:title>T</dc:title></feed>";
const MULTI: &str = "<one>first</one>\n<two>second</two>";
fn write(dir: &Path, name: &str, text: &str) -> PathBuf {
    let p = dir.join(name);
    fs::write(&p, text).unwrap();
    p
}
fn parse(text: &str) -> xml_combiner::Element {
    let d = tempdir().unwrap();
    parse_roots(&write(d.path(), "x.xml", text))
        .unwrap()
        .0
        .remove(0)
}
fn setup(files: &[(&str, &str)]) -> (TempDir, Options) {
    let d = tempdir().unwrap();
    let input = d.path().join("in");
    fs::create_dir(&input).unwrap();
    for (n, t) in files {
        write(&input, n, t);
    }
    let opt = Options::new(input, d.path().join("out.xml"));
    (d, opt)
}
fn run(files: &[(&str, &str)], change: impl FnOnce(&mut Options)) -> (String, Report) {
    let (_d, mut opt) = setup(files);
    change(&mut opt);
    let mut report = Report::default();
    combine(&opt, &mut report).unwrap();
    (fs::read_to_string(opt.output).unwrap(), report)
}
fn tags(s: &str) -> Vec<String> {
    parse(s).children.into_iter().map(|e| e.tag).collect()
}
#[test]
fn element_hash_is_stable_for_equal_content() {
    assert_eq!(
        element_key(&parse("<x a='1'>t</x>")),
        element_key(&parse("<x a='1'>t</x>"))
    );
}
#[test]
fn element_hash_separates_attribute_order_from_value() {
    let a = element_key(&parse("<x b='2' a='1'/>"));
    assert_eq!(a, element_key(&parse("<x a='1' b='2'/>")));
    assert_ne!(a, element_key(&parse("<x a='1' b='3'/>")));
}
#[test]
fn element_hash_covers_text_and_children() {
    let a = element_key(&parse("<x><c>1</c></x>"));
    assert_ne!(a, element_key(&parse("<x><c>2</c></x>")));
    assert_ne!(a, element_key(&parse("<x><c>1</c><c>1</c></x>")));
}
#[test]
fn declared_encoding_windows() {
    assert_eq!(
        declared_encoding(b"<?xml version=\"1.0\" encoding=\"windows-1250\"?><a/>"),
        "windows-1250"
    );
}
#[test]
fn declared_encoding_iso() {
    assert_eq!(
        declared_encoding(b"<?xml version='1.0' encoding='ISO-8859-2'?><a/>"),
        "ISO-8859-2"
    );
}
#[test]
fn declared_encoding_without_encoding() {
    assert_eq!(declared_encoding(b"<?xml version='1.0'?><a/>"), "utf-8");
}
#[test]
fn declared_encoding_without_declaration() {
    assert_eq!(declared_encoding(b"<a/>"), "utf-8");
}
#[test]
fn parse_roots_single_document() {
    assert_eq!(parse(SINGLE).tag, "root");
}
#[test]
fn parse_roots_keeps_every_root_of_a_multi_root_file() {
    let d = tempdir().unwrap();
    let (r, _) = parse_roots(&write(d.path(), "a.xml", MULTI)).unwrap();
    assert_eq!(
        r.iter().map(|e| e.tag.as_str()).collect::<Vec<_>>(),
        ["one", "two"]
    );
}
#[test]
fn parse_roots_collects_namespace_prefixes() {
    let d = tempdir().unwrap();
    let (_, p) = parse_roots(&write(d.path(), "a.xml", NS)).unwrap();
    assert_eq!(
        p,
        [("dc".into(), "http://purl.org/dc/elements/1.1/".into())]
    );
}
#[test]
fn parse_roots_collects_prefixes_from_multi_root_files() {
    let d = tempdir().unwrap();
    let (r, p) = parse_roots(&write(d.path(), "a.xml", &format!("{NS}\n<two/>"))).unwrap();
    assert_eq!(r.len(), 2);
    assert_eq!(p[0].0, "dc");
}
#[test]
fn parse_roots_honors_the_declared_encoding_on_the_wrapper_path() {
    let d = tempdir().unwrap();
    let p = d.path().join("a.xml");
    let (bytes, _, _) = encoding_rs::WINDOWS_1250
        .encode("<?xml version='1.0' encoding='windows-1250'?><a>Kopřiva</a>\n<b/>");
    fs::write(&p, bytes).unwrap();
    let (r, _) = parse_roots(&p).unwrap();
    assert_eq!(r.len(), 2);
    assert_eq!(r[0].text, "Kopřiva");
}
#[test]
fn parse_roots_strips_a_doctype_before_wrapping() {
    let d = tempdir().unwrap();
    let (r, _) = parse_roots(&write(
        d.path(),
        "a.xml",
        "<?xml version='1.0'?><!DOCTYPE note SYSTEM 'note.dtd'>\n<one/>\n<two/>",
    ))
    .unwrap();
    assert_eq!(r.len(), 2);
}
#[test]
fn parse_roots_raises_on_a_genuinely_malformed_document() {
    let d = tempdir().unwrap();
    assert!(parse_roots(&write(d.path(), "a.xml", "<root><unclosed></root>")).is_err());
}
#[test]
fn get_xml_files_ignores_other_extensions_and_is_case_insensitive() {
    let (_d, o) = setup(&[("a.xml", SINGLE), ("b.XML", SINGLE), ("notes.txt", "nope")]);
    let f = get_xml_files(&o.input, false).unwrap();
    assert_eq!(
        f.iter()
            .map(|p| p.file_name().unwrap().to_str().unwrap())
            .collect::<Vec<_>>(),
        ["a.xml", "b.XML"]
    );
}
#[test]
fn get_xml_files_walks_subdirectories_only_when_recursive() {
    let (_d, o) = setup(&[("top.xml", SINGLE)]);
    fs::create_dir(o.input.join("nested")).unwrap();
    write(&o.input.join("nested"), "deep.xml", SINGLE);
    assert_eq!(get_xml_files(&o.input, false).unwrap().len(), 1);
    assert_eq!(get_xml_files(&o.input, true).unwrap().len(), 2);
}
#[test]
fn combine_preserves_each_root_by_default() {
    assert_eq!(
        tags(&run(&[("a.xml", SINGLE), ("b.xml", SINGLE)], |_| {}).0),
        ["root", "root"]
    );
}
#[test]
fn flatten_lifts_the_children_out_of_each_root() {
    assert_eq!(
        tags(&run(&[("a.xml", SINGLE)], |o| o.flatten = true).0),
        ["item"]
    );
}
#[test]
fn deduplicate_drops_identical_elements_across_files() {
    assert_eq!(
        tags(
            &run(&[("a.xml", SINGLE), ("b.xml", SINGLE)], |o| o.deduplicate =
                true)
            .0
        ),
        ["root"]
    );
}
#[test]
fn multi_root_files_reach_the_output() {
    assert_eq!(tags(&run(&[("a.xml", MULTI)], |_| {}).0), ["one", "two"]);
}
#[test]
fn namespace_prefix_survives_the_round_trip() {
    let s = run(&[("a.xml", NS)], |_| {}).0;
    assert!(s.contains("xmlns:dc=\"http://purl.org/dc/elements/1.1/\""));
    assert!(!s.contains("ns0:"));
}
#[test]
fn a_malformed_file_fails_without_stopping_the_others() {
    let (_, r) = run(
        &[("good.xml", SINGLE), ("bad.xml", "<root><unclosed></root>")],
        |_| {},
    );
    assert_eq!((r.processed_files, r.failed_files), (1, 1));
}
#[test]
fn a_malformed_file_is_parsed_once_and_not_retried() {
    let (_d, o) = setup(&[("bad.xml", "<root><unclosed></root>")]);
    let mut r = Report::default();
    assert!(combine(&o, &mut r).is_err());
    assert_eq!(r.parse_attempts, 1);
}
#[test]
fn empty_input_folder_reports_failure() {
    let (_d, o) = setup(&[]);
    assert!(combine(&o, &mut Report::default()).is_err());
}
#[test]
fn missing_input_folder_reports_failure() {
    let (_d, mut o) = setup(&[]);
    o.input.push("nope");
    assert!(combine(&o, &mut Report::default())
        .unwrap_err()
        .contains("Input folder does not exist"));
}
#[test]
fn relative_output_may_not_escape_the_working_directory() {
    let (_d, mut o) = setup(&[("a.xml", SINGLE)]);
    o.output = "../escaped.xml".into();
    assert!(combine(&o, &mut Report::default())
        .unwrap_err()
        .contains("Refusing to write outside working directory"));
}
#[test]
fn absolute_output_outside_the_working_directory_is_allowed() {
    let (_d, o) = setup(&[("a.xml", SINGLE)]);
    assert!(o.output.is_absolute());
    combine(&o, &mut Report::default()).unwrap();
    assert!(o.output.exists());
}
#[test]
fn exact_xml_declaration_whitespace_attributes_and_escaping() {
    let s = run(
        &[(
            "a.xml",
            "<r z='&quot;' a='&amp;'>\n  <e/>tail &lt; &gt; &#13;</r>",
        )],
        |_| {},
    )
    .0;
    assert_eq!(s,"<?xml version='1.0' encoding='utf-8'?>\n<combined><r z=\"&quot;\" a=\"&amp;\">\n  <e />tail &lt; &gt; \r</r></combined>");
}
#[test]
fn deduplication_includes_child_tails() {
    let s = run(
        &[
            ("a.xml", "<p><b/>first</p>"),
            ("b.xml", "<p><b/>second</p>"),
        ],
        |o| o.deduplicate = true,
    )
    .0;
    assert_eq!(tags(&s).len(), 2);
}
#[test]
fn deduplication_has_no_separator_collisions() {
    let s = run(
        &[("a.xml", "<x>a|b=c</x>"), ("b.xml", "<x b='c'>a</x>")],
        |o| o.deduplicate = true,
    )
    .0;
    assert_eq!(tags(&s).len(), 2);
}
#[test]
fn entities_are_rejected_without_aborting_other_files() {
    let (_, r) = run(
        &[
            ("a.xml", "<!DOCTYPE x [<!ENTITY a 'secret'>]><x>&a;</x>"),
            ("b.xml", "<ok/>"),
        ],
        |_| {},
    );
    assert_eq!(r.failed_files, 1);
}
#[test]
fn empty_flattened_output_uses_short_element() {
    assert_eq!(
        run(&[("a.xml", "<root/>")], |o| o.flatten = true).0,
        "<?xml version='1.0' encoding='utf-8'?>\n<combined />"
    );
}
#[test]
fn output_can_also_be_an_input() {
    let (_d, mut o) = setup(&[("a.xml", SINGLE)]);
    o.output = o.input.join("a.xml");
    combine(&o, &mut Report::default()).unwrap();
    assert_eq!(tags(&fs::read_to_string(o.output).unwrap()), ["root"]);
}
#[test]
fn schema_option_is_only_a_well_formedness_gate() {
    let (_d, mut o) = setup(&[("a.xml", SINGLE)]);
    o.schema = Some(write(o.input.parent().unwrap(), "schema.xsd", "not an XSD"));
    combine(&o, &mut Report::default()).unwrap();
}
#[test]
fn zero_retries_processes_no_files() {
    let (_d, mut o) = setup(&[("a.xml", SINGLE)]);
    o.max_retries = 0;
    assert!(combine(&o, &mut Report::default()).is_err());
}
#[test]
fn utf16_is_decoded() {
    let d = tempdir().unwrap();
    let p = d.path().join("a.xml");
    let text = "<?xml version='1.0' encoding='utf-16'?><x>Kopřiva</x>";
    let mut bytes = vec![0xff, 0xfe];
    bytes.extend(text.encode_utf16().flat_map(u16::to_le_bytes));
    fs::write(&p, bytes).unwrap();
    assert_eq!(parse_roots(&p).unwrap().0[0].text, "Kopřiva");
}
#[test]
fn prefix_conflicts_match_elementtree() {
    let s = run(
        &[
            ("a.xml", "<p:a xmlns:p='urn:a'/>"),
            ("b.xml", "<p:b xmlns:p='urn:b'/>"),
        ],
        |_| {},
    )
    .0;
    assert_eq!(s,"<?xml version='1.0' encoding='utf-8'?>\n<combined xmlns:ns0=\"urn:a\" xmlns:p=\"urn:b\"><ns0:a /><p:b /></combined>");
}

fn tempdir() -> std::io::Result<tempfile::TempDir> {
    tempfile::tempdir_in(env!("CARGO_MANIFEST_DIR"))
}

#[test]
fn default_namespace_preserves_the_unnamespaced_wrapper_and_children() {
    let s = run(
        &[(
            "a.xml",
            "<root xmlns='urn:a'><child/><plain xmlns=''/></root>",
        )],
        |_| {},
    )
    .0;
    let root = parse(&s);
    assert_eq!(root.tag, "combined");
    assert_eq!(root.children[0].tag, "{urn:a}root");
    assert_eq!(root.children[0].children[1].tag, "plain");
    assert!(s.contains("xmlns:ns0=\"urn:a\""));
}
#[test]
fn default_namespace_cannot_strip_an_attributes_namespace() {
    let s = run(
        &[("a.xml", "<r xmlns='urn:a' xmlns:p='urn:a' p:id='1'/>")],
        |_| {},
    )
    .0;
    assert_eq!(parse(&s).children[0].attributes[0].0, "{urn:a}id");
}
#[test]
fn reserved_numbered_prefix_is_preserved_without_crashing() {
    let s = run(&[("a.xml", "<ns0:r xmlns:ns0='urn:a'/>")], |_| {}).0;
    assert!(s.contains("<ns0:r />"));
    assert_eq!(parse(&s).children[0].tag, "{urn:a}r");
}
#[test]
fn literal_and_referenced_attribute_whitespace_differ() {
    let s = run(&[("a.xml", "<r a='&#10;&#13;&#9;\n'/>")], |_| {}).0;
    assert!(s.contains("a=\"&#10;&#13;&#09; \""));
}
#[test]
fn single_root_with_outside_text_retains_its_wrapper_fallback_tail() {
    assert_eq!(
        run(&[("a.xml", "<r/> tail")], |_| {}).0,
        "<?xml version='1.0' encoding='utf-8'?>\n<combined><r /> tail</combined>"
    );
}
#[test]
fn invalid_root_names_fail_without_creating_output() {
    for root in ["", "1root", "a b", "p:unbound", "x><injected"] {
        let (_d, mut o) = setup(&[("a.xml", SINGLE)]);
        o.root = root.into();
        assert!(combine(&o, &mut Report::default()).is_err());
        assert!(!o.output.exists());
    }
}
#[test]
fn malformed_names_characters_and_declarations_fail() {
    for xml in [
        "<1root/>",
        "<a:b:c/>",
        "<r a='<bad'/>",
        "<r>&#0;</r>",
        "<r>\u{1}</r>",
        "<r><?xml version='1.0'?></r>",
        "<r><!DOCTYPE x></r>",
        "<r xmlns:xml='wrong'/>",
        "<r xmlns:p=''/>",
        "<r a='1' a='2'/>",
        "<r><!-- invalid--comment --></r>",
        "<r>bad]]>text</r>",
        "<r><?XML wrong?></r>",
    ] {
        let d = tempdir().unwrap();
        assert!(
            parse_roots(&write(d.path(), "a.xml", xml)).is_err(),
            "accepted {xml}"
        );
    }
}
#[test]
fn utf16_big_endian_multi_root_is_streamed() {
    let d = tempdir().unwrap();
    let p = d.path().join("a.xml");
    let text = "<?xml version='1.0' encoding='utf-16'?><x>Kopřiva</x>\n<y/>";
    let mut bytes = vec![0xfe, 0xff];
    bytes.extend(text.encode_utf16().flat_map(u16::to_be_bytes));
    fs::write(&p, bytes).unwrap();
    let (roots, _) = parse_roots(&p).unwrap();
    assert_eq!(roots.len(), 2);
    assert_eq!(roots[0].text, "Kopřiva");
}
#[test]
fn unknown_encoding_is_a_per_file_failure() {
    let (_, r) = run(
        &[
            ("a.xml", "<?xml version='1.0' encoding='nonexistent'?><r/>"),
            ("b.xml", SINGLE),
        ],
        |_| {},
    );
    assert_eq!((r.processed_files, r.failed_files), (1, 1));
}
#[test]
fn flatten_streams_many_children_and_keeps_their_tails() {
    use std::io::Write;
    let (_d, mut o) = setup(&[]);
    o.flatten = true;
    let mut file = std::io::BufWriter::new(fs::File::create(o.input.join("large.xml")).unwrap());
    file.write_all(b"<root>").unwrap();
    for _ in 0..20000 {
        file.write_all(b"<item>value</item>tail\n").unwrap();
    }
    file.write_all(b"</root>").unwrap();
    file.flush().unwrap();
    combine(&o, &mut Report::default()).unwrap();
    let mut reader =
        quick_xml::Reader::from_reader(std::io::BufReader::new(fs::File::open(o.output).unwrap()));
    let mut buf = vec![];
    let mut count = 0;
    loop {
        match reader.read_event_into(&mut buf).unwrap() {
            quick_xml::events::Event::Start(e) if e.name().as_ref() == b"item" => count += 1,
            quick_xml::events::Event::Eof => break,
            _ => {}
        }
        buf.clear();
    }
    assert_eq!(count, 20000);
}

#[test]
fn dtd_defaults_and_attribute_type_normalization_match_single_root_python() {
    let s=run(&[("a.xml", "<!DOCTYPE r [<!ATTLIST r a CDATA 'default' ids NMTOKENS #IMPLIED fixed CDATA #FIXED 'yes'>]><r ids='  a   b  '/>")], |_|{}).0;
    assert_eq!(s,"<?xml version='1.0' encoding='utf-8'?>\n<combined><r ids=\"a b\" a=\"default\" fixed=\"yes\" /></combined>");
}
#[test]
fn dtd_defaults_survive_multi_root_fragments() {
    let s = run(
        &[(
            "a.xml",
            "<!DOCTYPE r [<!ATTLIST r a CDATA 'default'>]><r/><r/>",
        )],
        |_| {},
    )
    .0;
    assert_eq!(
        parse(&s).children[0].attributes,
        [("a".into(), "default".into())]
    );
    assert_eq!(
        parse(&s).children[1].attributes,
        [("a".into(), "default".into())]
    );
}
#[test]
fn entity_text_in_a_dtd_comment_is_not_a_declaration() {
    let d = tempdir().unwrap();
    assert!(parse_roots(&write(
        d.path(),
        "a.xml",
        "<!DOCTYPE r [<!-- <!ENTITY harmless -->]><r/>"
    ))
    .is_ok());
}
#[test]
fn latin1_c1_bytes_are_not_decoded_as_windows_1252() {
    let d = tempdir().unwrap();
    let p = d.path().join("a.xml");
    fs::write(
        &p,
        b"<?xml version='1.0' encoding='ISO-8859-1'?><r>\x80\x85\xa0</r>",
    )
    .unwrap();
    assert_eq!(parse_roots(&p).unwrap().0[0].text, "\u{80}\u{85}\u{a0}");
}
#[test]
fn ascii_rejects_non_ascii_bytes() {
    let d = tempdir().unwrap();
    let p = d.path().join("a.xml");
    fs::write(&p, b"<?xml version='1.0' encoding='ASCII'?><r>\x80</r>").unwrap();
    assert!(parse_roots(&p).is_err());
}
#[test]
fn iso8859_2_is_decoded_on_single_and_multiple_roots() {
    let d = tempdir().unwrap();
    let p = d.path().join("a.xml");
    for tail in ["", "<b/>"] {
        let text = format!("<?xml version='1.0' encoding='ISO-8859-2'?><r>Kopřiva</r>{tail}");
        let (bytes, _, errors) = encoding_rs::ISO_8859_2.encode(&text);
        assert!(!errors);
        fs::write(&p, bytes).unwrap();
        assert_eq!(parse_roots(&p).unwrap().0[0].text, "Kopřiva");
    }
}

#[test]
fn utf16_without_bom_or_declaration_is_detected() {
    let d = tempdir().unwrap();
    let p = d.path().join("a.xml");
    for bytes in [
        "<r>ř</r>"
            .encode_utf16()
            .flat_map(u16::to_le_bytes)
            .collect::<Vec<_>>(),
        "<r>ř</r>"
            .encode_utf16()
            .flat_map(u16::to_be_bytes)
            .collect::<Vec<_>>(),
    ] {
        fs::write(&p, bytes).unwrap();
        assert_eq!(parse_roots(&p).unwrap().0[0].text, "ř");
    }
}

#[test]
fn streamed_and_deduplicated_serializers_preserve_the_same_mixed_content() {
    let text = "<r>discard<b>text<c/>nested</b>tail\n<d><!--ignored--></d>last</r>";
    for flatten in [false, true] {
        let ordinary = run(&[("a.xml", text)], |o| o.flatten = flatten).0;
        let dedup = run(&[("a.xml", text)], |o| {
            o.flatten = flatten;
            o.deduplicate = true;
        })
        .0;
        assert_eq!(ordinary, dedup);
        assert!(ordinary.contains("</b>tail\n<d />last"));
    }
}

#[test]
fn bound_prefixed_wrapper_names_are_preserved() {
    let s = run(&[("a.xml", "<p:r xmlns:p='urn:a'/>")], |o| {
        o.root = "p:wrapper".into()
    })
    .0;
    assert_eq!(parse(&s).tag, "{urn:a}wrapper");
}

#[test]
fn declared_encoding_matches_the_original_byte_pattern() {
    for (raw, expected) in [
        (b"<?xml encoding='latin1'".as_slice(), "latin1"),
        (b"<?xml encoding='invalid label'?>".as_slice(), "utf-8"),
        (b"\xc2\xa0<?xml encoding='latin1'?>".as_slice(), "utf-8"),
        (b"<?xml xencoding='ISO-8859-2'?>".as_slice(), "ISO-8859-2"),
    ] {
        assert_eq!(declared_encoding(raw), expected);
    }
}

#[test]
fn namespace_uris_in_wrapper_names_must_be_valid() {
    for root in ["{}r", "{\u{1}}r"] {
        let (_d, mut options) = setup(&[("a.xml", "<r/>")]);
        options.root = root.into();
        assert!(combine(&options, &mut Report::default()).is_err());
        assert!(!options.output.exists());
    }
}

#[test]
fn nesting_limit_is_checked_before_recursive_tree_operations() {
    let d = tempdir().unwrap();
    let path = write(
        d.path(),
        "depth.xml",
        &format!("{}{}", "<r>".repeat(256), "</r>".repeat(256)),
    );
    let roots = parse_roots(&path).unwrap().0;
    assert_eq!(element_key(&roots[0]).tag, "r");
    write(
        d.path(),
        "depth.xml",
        &format!("{}{}", "<r>".repeat(257), "</r>".repeat(257)),
    );
    assert!(parse_roots(&path).unwrap_err().contains("nesting"));
}
