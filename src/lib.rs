// Project: xml-combiner
// File: src/lib.rs
// Description: XML parsing, namespace reconciliation, and incremental combination.
// Author: Jan Alexandr Kopřiva
// License: MIT

use quick_xml::{events::Event, Reader};
use std::collections::{BTreeMap, HashSet};
use std::fs::{self, File};
use std::io::{self, BufReader, BufWriter, Write};
use std::path::{Component, Path, PathBuf};

pub fn log(level: &str, message: &str) {
    let seconds = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();
    let time = seconds as libc::time_t;
    let mut local = std::mem::MaybeUninit::<libc::tm>::uninit();
    // The C runtime supplies the same local timezone and DST rules as Python.
    #[cfg(unix)]
    let valid = unsafe { !libc::localtime_r(&time, local.as_mut_ptr()).is_null() };
    #[cfg(windows)]
    let valid = unsafe { libc::localtime_s(local.as_mut_ptr(), &time) == 0 };
    #[cfg(not(any(unix, windows)))]
    let valid = false;
    if valid {
        // A successful localtime call initializes every field used below.
        let local = unsafe { local.assume_init() };
        eprintln!(
            "{:04}-{:02}-{:02} {:02}:{:02}:{:02} - xml_combiner - {level} - {message}",
            local.tm_year + 1900,
            local.tm_mon + 1,
            local.tm_mday,
            local.tm_hour,
            local.tm_min,
            local.tm_sec
        );
        return;
    }
    let days = (seconds / 86400) as i64 + 719468;
    let era = days / 146097;
    let day = days - era * 146097;
    let year = (day - day / 1460 + day / 36524 - day / 146096) / 365;
    let ordinal = day - (365 * year + year / 4 - year / 100);
    let month = (5 * ordinal + 2) / 153;
    let date = ordinal - (153 * month + 2) / 5 + 1;
    let month = month + if month < 10 { 3 } else { -9 };
    let year = year + era * 400 + i64::from(month <= 2);
    let time = seconds % 86400;
    eprintln!(
        "{year:04}-{month:02}-{date:02} {:02}:{:02}:{:02} - xml_combiner - {level} - {message}",
        time / 3600,
        time / 60 % 60,
        time % 60
    );
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Hash)]
pub struct Element {
    pub tag: String,
    pub attributes: Vec<(String, String)>,
    pub text: String,
    pub children: Vec<Element>,
    pub tail: String,
}

pub fn element_key(e: &Element) -> Element {
    let mut e = e.clone();
    e.tail.clear();
    fn sort(e: &mut Element) {
        e.attributes.sort();
        for child in &mut e.children {
            sort(child);
        }
    }
    sort(&mut e);
    e
}

pub fn declared_encoding(raw: &[u8]) -> String {
    let raw = raw.trim_ascii_start();
    let Some(body) = raw.strip_prefix(b"<?xml") else {
        return "utf-8".into();
    };
    let body = &body[..body.iter().position(|b| *b == b'>').unwrap_or(body.len())];
    for index in 0..body.len() {
        let Some(rest) = body[index..].strip_prefix(b"encoding") else {
            continue;
        };
        let Some(rest) = rest.trim_ascii_start().strip_prefix(b"=") else {
            continue;
        };
        let rest = rest.trim_ascii_start();
        let Some((&quote, label)) = rest.split_first() else {
            continue;
        };
        if !matches!(quote, b'\'' | b'"') {
            continue;
        }
        let length = label
            .iter()
            .take_while(|b| b.is_ascii_alphanumeric() || matches!(b, b'_' | b'.' | b'-'))
            .count();
        if length > 0 && label.get(length) == Some(&quote) {
            return String::from_utf8_lossy(&label[..length]).into_owned();
        }
    }
    "utf-8".into()
}

type Prefixes = Vec<(String, String)>;
fn insert(map: &mut Prefixes, key: String, value: String) {
    if let Some(pair) = map.iter_mut().find(|(k, _)| *k == key) {
        pair.1 = value;
    } else {
        map.push((key, value));
    }
}
fn xml_start(c: char) -> bool {
    matches!(c, ':' | '_' | 'A'..='Z' | 'a'..='z' | '\u{c0}'..='\u{d6}' | '\u{d8}'..='\u{f6}' | '\u{f8}'..='\u{2ff}' | '\u{370}'..='\u{37d}' | '\u{37f}'..='\u{1fff}' | '\u{200c}'..='\u{200d}' | '\u{2070}'..='\u{218f}' | '\u{2c00}'..='\u{2fef}' | '\u{3001}'..='\u{d7ff}' | '\u{f900}'..='\u{fdcf}' | '\u{fdf0}'..='\u{fffd}' | '\u{10000}'..='\u{effff}')
}
fn valid_name(name: &str) -> bool {
    let mut parts = name.split(':');
    let valid = |s: &str| {
        let mut chars = s.chars();
        chars.next().is_some_and(|c| c != ':' && xml_start(c)) && chars.all(|c| c != ':' && (xml_start(c) || matches!(c, '-' | '.' | '0'..='9' | '\u{b7}' | '\u{300}'..='\u{36f}' | '\u{203f}'..='\u{2040}')))
    };
    valid(parts.next().unwrap_or("")) && parts.next().is_none_or(valid) && parts.next().is_none()
}
fn xml_whitespace(text: &str) -> bool {
    text.chars().all(|c| matches!(c, ' ' | '\t' | '\r' | '\n'))
}
fn separated_attributes(raw: &[u8]) -> bool {
    let mut quote = None;
    let mut closed = false;
    for &byte in raw {
        if let Some(q) = quote {
            if byte == q {
                quote = None;
                closed = true;
            }
        } else {
            if closed && !matches!(byte, b' ' | b'\t' | b'\r' | b'\n') {
                return false;
            }
            closed = false;
            if byte == b'\'' || byte == b'"' {
                quote = Some(byte);
            }
        }
    }
    true
}
fn valid_text(text: &str) -> Result<(), String> {
    if text.chars().all(|c| matches!(c, '\t' | '\n' | '\r' | '\u{20}'..='\u{d7ff}' | '\u{e000}'..='\u{fffd}' | '\u{10000}'..='\u{10ffff}')) { Ok(()) }
    else { Err("invalid XML character".into()) }
}
fn expanded(name: &str, namespaces: &Prefixes, attribute: bool) -> Result<String, String> {
    if !valid_name(name) {
        return Err(format!("invalid XML name: {name}"));
    }
    let (prefix, local) = name.split_once(':').unwrap_or(("", name));
    if prefix == "xml" {
        return Ok(format!("{{http://www.w3.org/XML/1998/namespace}}{local}"));
    }
    if attribute && prefix.is_empty() {
        return Ok(name.into());
    }
    match namespaces.iter().find(|(p, _)| p == prefix) {
        Some((_, uri)) if !uri.is_empty() => Ok(format!("{{{uri}}}{local}")),
        _ if prefix.is_empty() => Ok(local.into()),
        _ => Err(format!("unbound prefix: {prefix}")),
    }
}
fn append_text(stack: &mut [Element], roots: &mut [Element], text: &str) {
    if let Some(parent) = stack.last_mut() {
        if let Some(child) = parent.children.last_mut() {
            child.tail.push_str(text);
        } else {
            parent.text.push_str(text);
        }
    } else if let Some(root) = roots.last_mut() {
        root.tail.push_str(text);
    }
}

fn read_error(error: impl std::borrow::Borrow<io::Error>) -> String {
    let error = error.borrow();
    if error.kind() == io::ErrorKind::InvalidData {
        error.to_string()
    } else {
        format!("Read error: {error}")
    }
}
pub fn parse_roots(path: &Path) -> Result<(Vec<Element>, Prefixes), String> {
    let file = File::open(path).map_err(|e| e.to_string())?;
    parse_file(file)
}
type Sink<'a> = Option<(&'a mut dyn FnMut(Element) -> Result<(), String>, bool)>;
fn parse_file(file: File) -> Result<(Vec<Element>, Prefixes), String> {
    parse_file_with_sink(file, None).map(|(roots, prefixes, _)| (roots, prefixes))
}
enum Notice {
    Start(Element, bool),
    End,
    Text(String),
    Fragment,
}
type Observer<'a> = Option<&'a mut dyn FnMut(Notice) -> Result<(), String>>;
fn parse_file_with_sink(
    file: File,
    sink: Sink<'_>,
) -> Result<(Vec<Element>, Prefixes, usize), String> {
    parse_file_with_handlers(file, sink, None)
}
fn parse_file_with_handlers(
    file: File,
    sink: Sink<'_>,
    observer: Observer<'_>,
) -> Result<(Vec<Element>, Prefixes, usize), String> {
    use std::io::Read;
    let mut source: Box<dyn io::BufRead> = Box::new(BufReader::new(file));
    let first = source.fill_buf().map_err(read_error)?;
    let starts_decl = first
        .strip_prefix(b"\xef\xbb\xbf")
        .unwrap_or(first)
        .starts_with(b"<?xml");
    let mut header_label = None;
    if starts_decl && !first.contains(&b'>') {
        // A declaration can exceed the buffered reader's first chunk.
        let mut header = Vec::new();
        source.read_until(b'>', &mut header).map_err(read_error)?;
        header_label = Some(declared_encoding(
            header.strip_prefix(b"\xef\xbb\xbf").unwrap_or(&header),
        ));
        source = Box::new(BufReader::new(io::Cursor::new(header).chain(source)));
    }
    let first = source.fill_buf().map_err(read_error)?;
    let encoding = if first.starts_with(&[0xff, 0xfe]) || first.starts_with(&[b'<', 0]) {
        Some(encoding_rs::UTF_16LE)
    } else if first.starts_with(&[0xfe, 0xff]) || first.starts_with(&[0, b'<']) {
        Some(encoding_rs::UTF_16BE)
    } else {
        None
    };
    if let Some(encoding) = encoding {
        parse_transcoded(Transcoder::new(source, encoding), sink, observer)
    } else {
        let label = header_label.unwrap_or_else(|| {
            declared_encoding(first.strip_prefix(b"\xef\xbb\xbf").unwrap_or(first))
        });
        let normalized = label.to_ascii_lowercase().replace(['-', '_', '.'], "");
        let mode = match normalized.as_str() {
            "ascii" | "usascii" | "646" | "iso646us" | "ansix341968" | "ansix341986" | "cp367"
            | "ibm367" | "csascii" => Some(SingleByteMode::Ascii),
            "iso88591" | "isoir100" | "l1" | "latin1" | "cp819" | "ibm819" | "csisolatin1" => {
                Some(SingleByteMode::Latin1)
            }
            "iso88599" => Some(SingleByteMode::C1(encoding_rs::WINDOWS_1254)),
            "iso885911" => Some(SingleByteMode::C1(encoding_rs::WINDOWS_874)),
            _ => None,
        };
        if let Some(mode) = mode {
            parse_transcoded(
                SingleByteReader {
                    source,
                    mode,
                    pending: vec![],
                },
                sink,
                observer,
            )
        } else {
            parse_reader(source, sink, observer)
        }
    }
}
fn parse_transcoded<R: io::Read>(
    source: R,
    sink: Sink<'_>,
    observer: Observer<'_>,
) -> Result<(Vec<Element>, Prefixes, usize), String> {
    use std::io::{BufRead, Read};
    let mut decoded = BufReader::new(source);
    let mut prefix = vec![0; 5];
    let mut length = 0;
    while length < 5 {
        let n = decoded.read(&mut prefix[length..]).map_err(read_error)?;
        if n == 0 {
            break;
        }
        length += n;
    }
    prefix.truncate(length);
    if prefix == b"<?xml" {
        let mut declaration = Vec::new();
        decoded
            .read_until(b'>', &mut declaration)
            .map_err(read_error)?;
        // Keep declaration validation without asking quick-xml to decode UTF-8 as the old encoding.
        let mut original = prefix;
        original.extend(declaration);
        let text = std::str::from_utf8(&original).map_err(|e| e.to_string())?;
        let mut declaration_reader = Reader::from_str(text);
        match declaration_reader.read_event().map_err(|e| e.to_string())? {
            Event::Decl(d) => {
                d.version().map_err(|e| e.to_string())?;
            }
            _ => return Err("invalid XML declaration".into()),
        }
        let header = io::Cursor::new(b"<?xml version='1.0' encoding='utf-8'?>");
        parse_reader(BufReader::new(header.chain(decoded)), sink, observer)
    } else {
        parse_reader(
            BufReader::new(io::Cursor::new(prefix).chain(decoded)),
            sink,
            observer,
        )
    }
}
enum SingleByteMode {
    Ascii,
    Latin1,
    C1(&'static encoding_rs::Encoding),
}
struct SingleByteReader<R> {
    source: R,
    mode: SingleByteMode,
    pending: Vec<u8>,
}
impl<R: io::Read> io::Read for SingleByteReader<R> {
    fn read(&mut self, output: &mut [u8]) -> io::Result<usize> {
        let mut written = 0;
        while written < output.len() {
            if !self.pending.is_empty() {
                let count = self.pending.len().min(output.len() - written);
                output[written..written + count].copy_from_slice(&self.pending[..count]);
                self.pending.drain(..count);
                written += count;
                continue;
            }
            let mut byte = [0];
            if self.source.read(&mut byte)? == 0 {
                break;
            }
            let b = byte[0];
            let c = match self.mode {
                SingleByteMode::Ascii if b >= 128 => {
                    return Err(io::Error::new(io::ErrorKind::InvalidData, "non-ASCII byte"))
                }
                SingleByteMode::Ascii | SingleByteMode::Latin1 => char::from(b),
                SingleByteMode::C1(_) if (0x80..=0x9f).contains(&b) => char::from(b),
                SingleByteMode::C1(encoding) => {
                    let value = encoding
                        .decode_without_bom_handling_and_without_replacement(&byte)
                        .ok_or_else(|| {
                            io::Error::new(io::ErrorKind::InvalidData, "invalid encoded byte")
                        })?;
                    value.chars().next().unwrap()
                }
            };
            let mut utf8 = [0; 4];
            self.pending
                .extend_from_slice(c.encode_utf8(&mut utf8).as_bytes());
        }
        Ok(written)
    }
}

// quick-xml requires ASCII-compatible input; transcode UTF-16 in bounded chunks.
struct Transcoder<R> {
    source: R,
    decoder: encoding_rs::Decoder,
    input: [u8; 8192],
    start: usize,
    end: usize,
    eof: bool,
}
impl<R> Transcoder<R> {
    fn new(source: R, encoding: &'static encoding_rs::Encoding) -> Self {
        Self {
            source,
            decoder: encoding.new_decoder_with_bom_removal(),
            input: [0; 8192],
            start: 0,
            end: 0,
            eof: false,
        }
    }
}
impl<R: io::Read> io::Read for Transcoder<R> {
    fn read(&mut self, out: &mut [u8]) -> io::Result<usize> {
        if out.is_empty() {
            return Ok(0);
        }
        loop {
            if self.start == self.end && !self.eof {
                self.end = self.source.read(&mut self.input)?;
                self.start = 0;
                self.eof = self.end == 0;
            }
            let (_, read, written, errors) =
                self.decoder
                    .decode_to_utf8(&self.input[self.start..self.end], out, self.eof);
            self.start += read;
            if errors {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidData,
                    "invalid UTF-16 input",
                ));
            }
            if written > 0 || self.eof {
                return Ok(written);
            }
        }
    }
}

#[derive(Debug)]
struct DtdAttribute {
    element: String,
    name: String,
    cdata: bool,
    value: Option<String>,
}
fn dtd_tokens(mut text: &str) -> Result<Vec<(String, bool)>, String> {
    let mut tokens = vec![];
    loop {
        text = text.trim_start();
        let Some(first) = text.chars().next() else {
            break;
        };
        if first == '\'' || first == '"' {
            let end = text[1..].find(first).ok_or("unclosed DTD string")? + 1;
            tokens.push((text[1..end].into(), true));
            text = &text[end + 1..];
        } else if first == '(' {
            let end = text.find(')').ok_or("unclosed DTD enumeration")?;
            tokens.push((text[..=end].into(), false));
            text = &text[end + 1..];
        } else {
            let end = text.find(char::is_whitespace).unwrap_or(text.len());
            tokens.push((text[..end].into(), false));
            text = &text[end..];
        }
    }
    Ok(tokens)
}
fn dtd_attributes(mut text: &str) -> Result<Vec<DtdAttribute>, String> {
    let mut attributes = vec![];
    while let Some(index) = text.find("<!") {
        text = &text[index..];
        if text.starts_with("<!--") {
            let end = text.find("-->").ok_or("unclosed DTD comment")?;
            text = &text[end + 3..];
            continue;
        }
        let mut quote = None;
        let mut end = None;
        for (index, c) in text.char_indices().skip(2) {
            if let Some(q) = quote {
                if c == q {
                    quote = None;
                }
            } else if c == '\'' || c == '"' {
                quote = Some(c);
            } else if c == '>' {
                end = Some(index);
                break;
            }
        }
        let end = end.ok_or("unclosed DTD declaration")?;
        let declaration = &text[2..end];
        text = &text[end + 1..];
        if declaration.starts_with("ENTITY") {
            return Err("EntitiesForbidden".into());
        }
        let Some(body) = declaration.strip_prefix("ATTLIST") else {
            continue;
        };
        let tokens = dtd_tokens(body)?;
        let Some((element, _)) = tokens.first() else {
            return Err("empty ATTLIST".into());
        };
        let mut index = 1;
        while index < tokens.len() {
            let name = tokens[index].0.clone();
            index += 1;
            let kind = tokens.get(index).ok_or("missing attribute type")?.0.clone();
            index += 1;
            if kind == "NOTATION" {
                index += 1;
            }
            let mut default = tokens.get(index).ok_or("missing attribute default")?;
            index += 1;
            if default.0 == "#FIXED" {
                default = tokens.get(index).ok_or("missing fixed value")?;
                index += 1;
            }
            let value = if default.1 {
                let normalized = default
                    .0
                    .replace("\r\n", "\n")
                    .replace(['\n', '\r', '\t'], " ");
                let value = quick_xml::escape::unescape(&normalized)
                    .map_err(|e| e.to_string())?
                    .into_owned();
                Some(value)
            } else if default.0 == "#REQUIRED" || default.0 == "#IMPLIED" {
                None
            } else {
                return Err("invalid attribute default".into());
            };
            if !attributes
                .iter()
                .any(|a: &DtdAttribute| a.element == *element && a.name == name)
            {
                attributes.push(DtdAttribute {
                    element: element.clone(),
                    name,
                    cdata: kind == "CDATA",
                    value,
                });
            }
        }
    }
    Ok(attributes)
}
// DTD comments do not contribute angle brackets to markup nesting. Normalize
// their discarded contents before quick-xml's angle-counting DOCTYPE reader.
struct DtdCommentFilter<R> {
    source: R,
    tail: Vec<u8>,
    doctype: bool,
    comment: bool,
    attlist: bool,
    quote: Option<u8>,
    brackets: usize,
    passthrough: bool,
    pending: std::collections::VecDeque<u8>,
}
impl<R: io::BufRead> io::Read for DtdCommentFilter<R> {
    fn read(&mut self, out: &mut [u8]) -> io::Result<usize> {
        if self.passthrough && self.pending.is_empty() {
            return self.source.read(out);
        }
        let mut written = 0;
        while written < out.len() {
            if let Some(byte) = self.pending.pop_front() {
                out[written] = byte;
                written += 1;
                continue;
            }
            let input = self.source.fill_buf()?;
            if input.is_empty() {
                break;
            }
            let byte = input[0];
            self.source.consume(1);
            let mut normalized = byte;
            if self.doctype {
                if self.comment {
                    if byte == b'>' && self.tail.ends_with(b"--") {
                        self.comment = false;
                    } else if byte == b'<' || byte == b'>' {
                        normalized = b' ';
                    }
                } else if let Some(quote) = self.quote {
                    if byte == quote {
                        self.quote = None;
                    } else if byte == b'<' && self.attlist {
                        return Err(io::Error::new(
                            io::ErrorKind::InvalidData,
                            "unescaped < in DTD attribute default",
                        ));
                    } else if byte == b'>' || byte == b'<' {
                        // quick-xml counts angle brackets even inside DTD literals.
                        normalized = b'&';
                        self.pending
                            .extend(if byte == b'>' { b"gt;" } else { b"lt;" });
                    }
                } else {
                    match byte {
                        b'\'' | b'"' => self.quote = Some(byte),
                        b'[' => self.brackets += 1,
                        b']' => self.brackets = self.brackets.saturating_sub(1),
                        b'>' => {
                            self.attlist = false;
                            if self.brackets == 0 {
                                self.doctype = false;
                            }
                        }
                        _ => {}
                    }
                }
            } else if self.tail.last() == Some(&b'<') && !matches!(byte, b'!' | b'?' | b'/') {
                self.passthrough = true;
            }
            self.tail.push(byte);
            if self.tail.len() > 9 {
                self.tail.remove(0);
            }
            if self.doctype && self.tail.ends_with(b"<!ATTLIST") {
                self.attlist = true;
            }
            if !self.doctype && self.tail.ends_with(b"<!DOCTYPE") {
                self.doctype = true;
            }
            if self.doctype && self.quote.is_none() && self.tail.ends_with(b"<!--") {
                self.comment = true;
            }
            out[written] = normalized;
            written += 1;
            if self.passthrough {
                break;
            }
        }
        Ok(written)
    }
}
fn parse_reader<R: io::BufRead>(
    source: R,
    mut sink: Sink<'_>,
    mut observer: Observer<'_>,
) -> Result<(Vec<Element>, Prefixes, usize), String> {
    let filtered = DtdCommentFilter {
        source,
        tail: vec![],
        doctype: false,
        comment: false,
        attlist: false,
        quote: None,
        brackets: 0,
        passthrough: false,
        pending: std::collections::VecDeque::new(),
    };
    let mut reader = Reader::from_reader(BufReader::new(filtered));
    reader.config_mut().check_end_names = true;
    reader.config_mut().check_comments = true;
    let mut buf = Vec::new();
    let mut stack: Vec<Element> = Vec::new();
    let mut scopes: Vec<Prefixes> = vec![vec![]];
    let mut roots: Vec<Element> = Vec::new();
    let mut root_count = 0;
    let mut outside_text = false;
    let mut declaration_seen = false;
    let mut misc_seen = false;
    let mut doctype_seen = false;
    let mut dtd = vec![];
    let mut prefixes = vec![];
    loop {
        let event = reader.read_event_into(&mut buf).map_err(|e| match e {
            quick_xml::Error::Io(e) => read_error(e.as_ref()),
            other => other.to_string(),
        })?;
        match event {
            Event::Start(ref start) | Event::Empty(ref start) => {
                let empty = matches!(event, Event::Empty(_));
                if !separated_attributes(start.attributes_raw()) {
                    return Err("attributes must be separated by whitespace".into());
                }
                // Tree cloning, hashing and dropping recurse; bound untrusted nesting.
                if stack.len() >= 256 {
                    return Err("XML nesting exceeds the limit of 256 elements".into());
                }
                if stack.is_empty() {
                    if let Some((callback, sink_flatten)) = sink.as_mut() {
                        for node in roots.drain(..) {
                            if !*sink_flatten {
                                callback(node)?;
                            }
                        }
                    }
                    root_count += 1;
                }
                if stack.len() == 1 {
                    if let Some((callback, true)) = sink.as_mut() {
                        for child in stack[0].children.drain(..) {
                            callback(child)?;
                        }
                    }
                }
                let mut ns = scopes.last().cloned().unwrap_or_default();
                let name = reader
                    .decoder()
                    .decode(start.name().as_ref())
                    .map_err(|e| e.to_string())?
                    .into_owned();
                let mut raw_attributes = vec![];
                for a in start.attributes() {
                    let a = a.map_err(|e| e.to_string())?;
                    let key = reader
                        .decoder()
                        .decode(a.key.as_ref())
                        .map_err(|e| e.to_string())?
                        .into_owned();
                    let raw = reader
                        .decoder()
                        .decode(a.value.as_ref())
                        .map_err(|e| e.to_string())?;
                    if raw.contains('<') {
                        return Err("unescaped < in attribute".into());
                    }
                    let normalized = raw.replace("\r\n", "\n").replace(['\n', '\r', '\t'], " ");
                    let value = quick_xml::escape::unescape(&normalized)
                        .map_err(|e| e.to_string())?
                        .into_owned();
                    raw_attributes.push((key, value));
                }
                for attribute in dtd.iter().filter(|a: &&DtdAttribute| a.element == name) {
                    if !raw_attributes.iter().any(|(k, _)| k == &attribute.name) {
                        if let Some(value) = &attribute.value {
                            raw_attributes.push((attribute.name.clone(), value.clone()));
                        }
                    }
                    if !attribute.cdata {
                        if let Some((_, v)) = raw_attributes
                            .iter_mut()
                            .find(|(k, _)| k == &attribute.name)
                        {
                            *v = v
                                .split(' ')
                                .filter(|s| !s.is_empty())
                                .collect::<Vec<_>>()
                                .join(" ");
                        }
                    }
                }
                let mut attrs = vec![];
                for (key, value) in raw_attributes {
                    valid_text(&value)?;
                    if key == "xmlns" || key.starts_with("xmlns:") {
                        let p = key.strip_prefix("xmlns:").unwrap_or("").to_owned();
                        if (!p.is_empty()
                            && (!valid_name(&p) || p.contains(':') || value.is_empty()))
                            || p == "xmlns"
                            || value.contains('}')
                            || value == "http://www.w3.org/2000/xmlns/"
                            || ((p == "xml") != (value == "http://www.w3.org/XML/1998/namespace"))
                        {
                            return Err("invalid namespace binding".into());
                        }
                        insert(&mut ns, p.clone(), value.clone());
                        insert(&mut prefixes, p, value);
                    } else {
                        attrs.push((key, value));
                    }
                }
                let tag = expanded(&name, &ns, false)?;
                let attributes = attrs
                    .into_iter()
                    .map(|(k, v)| Ok((expanded(&k, &ns, true)?, v)))
                    .collect::<Result<Vec<_>, String>>()?;
                let mut keys = HashSet::new();
                if attributes.iter().any(|(k, _)| !keys.insert(k)) {
                    return Err("duplicate attribute".into());
                }
                let node = Element {
                    tag,
                    attributes,
                    ..Element::default()
                };
                if let Some(notify) = observer.as_mut() {
                    notify(Notice::Start(node.clone(), empty))?;
                }
                if empty {
                    if observer.is_none() {
                        if let Some(parent) = stack.last_mut() {
                            parent.children.push(node);
                        } else {
                            roots.push(node);
                        }
                    }
                } else {
                    stack.push(node);
                    scopes.push(ns);
                }
            }
            Event::End(_) => {
                if stack.len() == 1 {
                    if let Some((callback, true)) = sink.as_mut() {
                        for child in stack[0].children.drain(..) {
                            callback(child)?;
                        }
                    }
                }
                let node = stack.pop().ok_or("unexpected closing tag")?;
                scopes.pop();
                if let Some(notify) = observer.as_mut() {
                    notify(Notice::End)?;
                } else if let Some(parent) = stack.last_mut() {
                    parent.children.push(node);
                } else {
                    roots.push(node);
                }
            }
            Event::Text(t) => {
                if t.as_ref().windows(3).any(|s| s == b"]]>") {
                    return Err("unexpected ]]> in character data".into());
                }
                let value = t.xml10_content().map_err(|e| e.to_string())?;
                let value = quick_xml::escape::unescape(&value).map_err(|e| e.to_string())?;
                valid_text(&value)?;
                if stack.is_empty() && !xml_whitespace(&value) {
                    outside_text = true;
                }
                if let Some(notify) = observer.as_mut() {
                    notify(Notice::Text(value.into_owned()))?;
                } else {
                    append_text(&mut stack, &mut roots, &value);
                }
            }
            Event::CData(t) => {
                if stack.is_empty() {
                    outside_text = true;
                    if let Some(notify) = observer.as_mut() {
                        notify(Notice::Fragment)?;
                    }
                }
                let value = t.xml10_content().map_err(|e| e.to_string())?;
                valid_text(&value)?;
                if stack.is_empty() && !xml_whitespace(&value) {
                    outside_text = true;
                }
                if let Some(notify) = observer.as_mut() {
                    notify(Notice::Text(value.into_owned()))?;
                } else {
                    append_text(&mut stack, &mut roots, &value);
                }
            }
            Event::GeneralRef(r) => {
                if stack.is_empty() {
                    outside_text = true;
                    if let Some(notify) = observer.as_mut() {
                        notify(Notice::Fragment)?;
                    }
                }
                let name = r.decode().map_err(|e| e.to_string())?;
                let encoded = format!("&{name};");
                let value = quick_xml::escape::unescape(&encoded).map_err(|e| e.to_string())?;
                valid_text(&value)?;
                if stack.is_empty() && !xml_whitespace(&value) {
                    outside_text = true;
                }
                if let Some(notify) = observer.as_mut() {
                    notify(Notice::Text(value.into_owned()))?;
                } else {
                    append_text(&mut stack, &mut roots, &value);
                }
            }
            Event::Comment(comment) => {
                valid_text(&comment.xml10_content().map_err(|e| e.to_string())?)?;
                misc_seen = true;
            }
            Event::PI(pi) => {
                valid_text(
                    &reader
                        .decoder()
                        .decode(pi.as_ref())
                        .map_err(|e| e.to_string())?,
                )?;
                misc_seen = true;
                let target = reader
                    .decoder()
                    .decode(pi.target())
                    .map_err(|e| e.to_string())?;
                if !valid_name(&target) || target.eq_ignore_ascii_case("xml") {
                    return Err("invalid processing instruction target".into());
                }
            }
            Event::Decl(d) => {
                if declaration_seen
                    || misc_seen
                    || outside_text
                    || doctype_seen
                    || root_count != 0
                    || !stack.is_empty()
                {
                    return Err("XML declaration not at start of document".into());
                }
                declaration_seen = true;
                d.version().map_err(|e| e.to_string())?;
                if let Some(encoding) = d.encoding() {
                    let encoding = encoding.map_err(|e| e.to_string())?;
                    if encoding_rs::Encoding::for_label(&encoding).is_none() {
                        return Err("unknown encoding".into());
                    }
                }
            }
            Event::DocType(t) => {
                if doctype_seen || root_count != 0 || !stack.is_empty() {
                    return Err("misplaced DOCTYPE".into());
                }
                doctype_seen = true;
                let s = t.decode().map_err(|e| e.to_string())?;
                dtd = dtd_attributes(&s)?;
            }
            Event::Eof => break,
        }
        buf.clear();
    }
    if !stack.is_empty() {
        return Err("unclosed element".into());
    }
    if root_count == 1 && !outside_text {
        if let Some(root) = roots.last_mut() {
            root.tail.clear();
        }
    }
    if let Some((callback, flatten)) = sink.as_mut() {
        for node in roots.drain(..) {
            if !*flatten {
                callback(node)?;
            }
        }
    }
    Ok((roots, prefixes, root_count))
}

#[derive(Debug, Clone)]
pub struct Options {
    pub input: PathBuf,
    pub output: PathBuf,
    pub root: String,
    pub recursive: bool,
    pub schema: Option<PathBuf>,
    pub deduplicate: bool,
    pub flatten: bool,
    pub max_retries: i64,
    pub verbose: bool,
}
impl Options {
    pub fn new(input: PathBuf, output: PathBuf) -> Self {
        Self {
            input,
            output,
            root: "combined".into(),
            recursive: false,
            schema: None,
            deduplicate: false,
            flatten: false,
            max_retries: 3,
            verbose: false,
        }
    }
}
#[derive(Debug, Default)]
pub struct Report {
    pub processed_files: usize,
    pub failed_files: usize,
    pub parse_attempts: usize,
}

pub fn get_xml_files(input: &Path, recursive: bool) -> io::Result<Vec<PathBuf>> {
    fn visit(dir: &Path, recursive: bool, files: &mut Vec<PathBuf>) -> io::Result<()> {
        let entries = match fs::read_dir(dir) {
            Err(error) if recursive && error.kind() == io::ErrorKind::PermissionDenied => {
                return Ok(())
            }
            result => result?,
        };
        for entry in entries {
            let entry = entry?;
            let path = entry.path();
            if path.is_file()
                && path
                    .extension()
                    .is_some_and(|e| e.eq_ignore_ascii_case("xml"))
            {
                files.push(path);
            } else if recursive && entry.file_type()?.is_dir() {
                visit(&path, recursive, files)?;
            }
        }
        Ok(())
    }
    let mut files = vec![];
    visit(input, recursive, &mut files)?;
    files.sort();
    Ok(files)
}

fn resolve_path(path: &Path, depth: usize) -> io::Result<PathBuf> {
    if depth > 40 {
        return Err(io::Error::other("too many symbolic links"));
    }
    let mut resolved = PathBuf::new();
    for part in path.components() {
        match part {
            Component::ParentDir => {
                resolved.pop();
            }
            Component::CurDir => {}
            other => {
                resolved.push(other.as_os_str());
                if let Ok(metadata) = fs::symlink_metadata(&resolved) {
                    if metadata.file_type().is_symlink() {
                        let target = fs::read_link(&resolved)?;
                        resolved.pop();
                        resolved = resolve_path(&resolved.join(target), depth + 1)?;
                    } else {
                        resolved = resolved.canonicalize()?;
                    }
                }
            }
        }
    }
    Ok(resolved)
}
pub fn safe_output(output: &Path) -> io::Result<PathBuf> {
    let base = std::env::current_dir()?.canonicalize()?;
    let path = if output.is_absolute() {
        output.to_owned()
    } else {
        base.join(output)
    };
    let resolved = resolve_path(&path, 0)?;
    if !output.is_absolute() && !resolved.starts_with(base) {
        return Err(io::Error::other(format!(
            "Refusing to write outside working directory: {} -> {}",
            output.display(),
            resolved.display()
        )));
    }
    Ok(resolved)
}

fn escape(s: &str, attribute: bool) -> String {
    let s = s
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;");
    if attribute {
        s.replace('"', "&quot;")
            .replace('\t', "&#09;")
            .replace('\n', "&#10;")
            .replace('\r', "&#13;")
    } else {
        s
    }
}
fn qualified(name: &str, namespaces: &Prefixes) -> String {
    if let Some((uri, local)) = name.strip_prefix('{').and_then(|s| s.rsplit_once('}')) {
        if uri == "http://www.w3.org/XML/1998/namespace" {
            return format!("xml:{local}");
        }
        let prefix = namespaces
            .iter()
            .find(|(_, u)| u == uri)
            .map(|(p, _)| p.as_str())
            .unwrap_or("");
        if prefix.is_empty() {
            local.into()
        } else {
            format!("{prefix}:{local}")
        }
    } else {
        name.into()
    }
}
fn collect_names(e: &Element, names: &mut Vec<String>) {
    if !names.contains(&e.tag) {
        names.push(e.tag.clone());
    }
    for (k, _) in &e.attributes {
        if !names.contains(k) {
            names.push(k.clone());
        }
    }
    for child in &e.children {
        collect_names(child, names);
    }
}
fn serialize<W: Write>(
    e: &Element,
    ns: &Prefixes,
    out: &mut W,
    declarations: bool,
) -> io::Result<()> {
    let name = qualified(&e.tag, ns);
    write!(out, "<{name}")?;
    if declarations {
        let sorted: BTreeMap<_, _> = ns.iter().filter(|(p, _)| p != "xml").cloned().collect();
        for (p, u) in sorted {
            write!(
                out,
                " xmlns{}=\"{}\"",
                if p.is_empty() {
                    String::new()
                } else {
                    format!(":{p}")
                },
                escape(&u, true)
            )?;
        }
    }
    for (k, v) in &e.attributes {
        write!(out, " {}=\"{}\"", qualified(k, ns), escape(v, true))?;
    }
    if e.text.is_empty() && e.children.is_empty() {
        write!(out, " />")?;
    } else {
        write!(out, ">{}", escape(&e.text, false))?;
        for child in &e.children {
            serialize(child, ns, out, false)?;
        }
        write!(out, "</{name}>")?;
    }
    write!(out, "{}", escape(&e.tail, false))
}

struct StreamFrame {
    node: Element,
    visible: bool,
    opened: bool,
    child_seen: bool,
}
struct StreamingSerializer<'a, W> {
    out: &'a mut W,
    ns: &'a Prefixes,
    flatten: bool,
    fragment: bool,
    frames: Vec<StreamFrame>,
    root_seen: bool,
    count: usize,
}
fn stream_open<W: Write>(frame: &mut StreamFrame, ns: &Prefixes, out: &mut W) -> io::Result<()> {
    if !frame.opened {
        let mut head = Vec::new();
        serialize(&frame.node, ns, &mut head, false)?;
        head.truncate(head.len() - 3);
        out.write_all(&head)?;
        out.write_all(b">")?;
        frame.opened = true;
    }
    Ok(())
}
impl<W: Write> StreamingSerializer<'_, W> {
    fn accept(&mut self, notice: Notice) -> io::Result<()> {
        match notice {
            Notice::Fragment => {}
            Notice::Start(node, empty) => {
                if let Some(parent) = self.frames.last_mut() {
                    parent.child_seen = true;
                    if parent.visible {
                        stream_open(parent, self.ns, self.out)?;
                    }
                }
                let depth = self.frames.len();
                let visible = !self.flatten || depth > 0;
                if (!self.flatten && depth == 0) || (self.flatten && depth == 1) {
                    self.count += 1;
                }
                if depth == 0 {
                    self.root_seen = true;
                }
                self.frames.push(StreamFrame {
                    node,
                    visible,
                    opened: false,
                    child_seen: false,
                });
                if empty {
                    self.accept(Notice::End)?;
                }
            }
            Notice::End => {
                let frame = self.frames.pop().expect("parser emits balanced events");
                if frame.visible {
                    if frame.opened {
                        write!(self.out, "</{}>", qualified(&frame.node.tag, self.ns))?;
                    } else {
                        serialize(&frame.node, self.ns, self.out, false)?;
                    }
                }
            }
            Notice::Text(text) => {
                if let Some(frame) = self.frames.last_mut() {
                    if !text.is_empty() && (frame.visible || frame.child_seen) {
                        if frame.visible {
                            stream_open(frame, self.ns, self.out)?;
                        }
                        write!(self.out, "{}", escape(&text, false))?;
                    }
                } else if !self.flatten && self.fragment && self.root_seen {
                    write!(self.out, "{}", escape(&text, false))?;
                }
            }
        }
        Ok(())
    }
}

pub fn combine(options: &Options, report: &mut Report) -> Result<(), String> {
    let local_root = options
        .root
        .strip_prefix('{')
        .and_then(|s| s.rsplit_once('}'))
        .map(|(_, name)| name)
        .unwrap_or(&options.root);
    let invalid_uri = options
        .root
        .strip_prefix('{')
        .and_then(|s| s.rsplit_once('}'))
        .is_some_and(|(uri, _)| uri.is_empty() || valid_text(uri).is_err());
    if invalid_uri
        || !valid_name(local_root)
        || (options.root.starts_with('{') && local_root.contains(':'))
    {
        return Err(format!("Invalid root element name: {}", options.root));
    }
    if !options.input.exists() {
        return Err(format!(
            "Input folder does not exist: {}",
            options.input.display()
        ));
    }
    if !options.input.is_dir() {
        return Err(format!(
            "Path is not a directory: {}",
            options.input.display()
        ));
    }
    let mut files = get_xml_files(&options.input, options.recursive).map_err(|e| e.to_string())?;
    // A previous run's output inside the input folder must not be combined into the next one.
    if let Ok(output) = safe_output(&options.output) {
        files.retain(|f| f.canonicalize().map_or(true, |f| f != output));
    }
    log("INFO", &format!("Found {} XML files", files.len()));
    if files.is_empty() {
        log("WARNING", "No XML files found to process");
        return Err(String::new());
    }
    *report = Report::default();
    let mut valid = vec![];
    let mut registered: Prefixes = vec![];
    let mut names = vec![options.root.clone()];
    for path in &files {
        if let Some(schema) = &options.schema {
            if !schema.exists() {
                log(
                    "WARNING",
                    &format!(
                        "Schema file not found, skipping validation: {}",
                        schema.display()
                    ),
                );
            }
        }
        let mut parsed = None;
        let mut file_names = vec![];
        let mut fragment = false;
        for attempt in 1..=options.max_retries {
            report.parse_attempts += 1;
            // Retry only opening/read errors, never deterministic XML errors.
            file_names.clear();
            fragment = false;
            let result = match File::open(path) {
                Ok(file) => {
                    let mut depth = 0;
                    let mut collect = |notice: Notice| {
                        match notice {
                            Notice::Fragment => fragment = true,
                            Notice::Start(node, empty) => {
                                if !options.flatten || depth > 0 {
                                    collect_names(&node, &mut file_names);
                                }
                                if !empty {
                                    depth += 1;
                                }
                            }
                            Notice::End => {
                                depth -= 1;
                            }
                            Notice::Text(text) => {
                                if depth == 0 && !xml_whitespace(&text) {
                                    fragment = true;
                                }
                            }
                        }
                        Ok(())
                    };
                    parse_file_with_handlers(file, None, Some(&mut collect))
                }
                Err(e) => Err(format!("Read error: {e}")),
            };
            if let Err(e) = &result {
                if let Some(e) = e.strip_prefix("Read error: ") {
                    if attempt < options.max_retries {
                        log(
                            "WARNING",
                            &format!(
                                "Read error (attempt {attempt}/{}) on {}: {e}",
                                options.max_retries,
                                path.file_name().unwrap_or_default().to_string_lossy()
                            ),
                        );
                        continue;
                    }
                    log(
                        "ERROR",
                        &format!(
                            "Cannot read {} after {} attempts",
                            path.file_name().unwrap_or_default().to_string_lossy(),
                            options.max_retries
                        ),
                    );
                }
            }
            parsed = Some(result);
            break;
        }
        match parsed {
            Some(Ok((_, prefixes, root_count))) => {
                if root_count > 1 {
                    log(
                        "WARNING",
                        &format!(
                            "{} has {} root elements, keeping all of them",
                            path.file_name().unwrap_or_default().to_string_lossy(),
                            root_count
                        ),
                    );
                }
                for (p, u) in prefixes {
                    if !registered.iter().any(|(_, v)| v == &u) {
                        insert(&mut registered, p, u);
                    }
                }

                for name in file_names {
                    if !names.contains(&name) {
                        names.push(name);
                    }
                }
                valid.push((path, fragment || root_count > 1));
                report.processed_files += 1;
                if options.verbose && options.schema.as_ref().is_some_and(|p| p.exists()) {
                    log(
                        "DEBUG",
                        &format!(
                            "Validated {}",
                            path.file_name().unwrap_or_default().to_string_lossy()
                        ),
                    );
                }
                if options.verbose {
                    log(
                        "DEBUG",
                        &format!(
                            "Processed file: {}",
                            path.file_name().unwrap_or_default().to_string_lossy()
                        ),
                    );
                }
            }
            other => {
                report.failed_files += 1;
                if let Some(Err(e)) = other {
                    if !e.starts_with("Read error: ") {
                        log(
                            "ERROR",
                            &format!(
                                "{} {}: {e}",
                                if options.schema.as_ref().is_some_and(|p| p.exists()) {
                                    "Validation failed for"
                                } else {
                                    "Malformed XML in"
                                },
                                path.file_name().unwrap_or_default().to_string_lossy()
                            ),
                        );
                    }
                }
            }
        }
    }
    log(
        "INFO",
        &format!(
            "Successfully processed {} of {} files",
            report.processed_files,
            files.len()
        ),
    );
    if report.failed_files > 0 {
        log(
            "WARNING",
            &format!("Failed to process {} files", report.failed_files),
        );
    }
    if valid.is_empty() {
        return Err(String::new());
    }
    let mut ns: Prefixes = vec![];
    for name in names {
        if let Some((uri, _)) = name.strip_prefix('{').and_then(|s| s.rsplit_once('}')) {
            if ns.iter().any(|(_, u)| u == uri) {
                continue;
            }
            if uri == "http://www.w3.org/XML/1998/namespace" {
                ns.push(("xml".into(), uri.into()));
                continue;
            }
            let preferred = registered
                .iter()
                .find(|(_, u)| u == uri)
                .map(|(p, _)| p.clone());
            let prefix = preferred.filter(|p| !p.is_empty()).unwrap_or_else(|| {
                let mut index = ns.len();
                loop {
                    let candidate = format!("ns{index}");
                    if !ns
                        .iter()
                        .chain(registered.iter())
                        .any(|(p, _)| p == &candidate)
                    {
                        break candidate;
                    }
                    index += 1;
                }
            });
            ns.push((prefix, uri.into()));
        }
    }
    if !options.root.starts_with('{') {
        if let Some((prefix, _)) = options.root.split_once(':') {
            if prefix != "xml" && !ns.iter().any(|(p, _)| p == prefix) {
                return Err(format!("Invalid root element name: {}", options.root));
            }
        }
    }
    let output = safe_output(&options.output).map_err(|e| e.to_string())?;
    if let Some(parent) = output.parent() {
        fs::create_dir_all(parent)
            .map_err(|e| format!("Error saving file {}: {e}", output.display()))?;
    }
    let builder = tempfile::Builder::new();
    #[cfg(unix)]
    let builder = {
        use std::os::unix::fs::PermissionsExt;
        let mut builder = builder;
        builder.permissions(fs::Permissions::from_mode(0o666));
        builder
    };
    let mut staging = builder
        .tempfile_in(output.parent().unwrap())
        .map_err(|e| format!("Error saving file {}: {e}", output.display()))?;
    if let Ok(metadata) = fs::metadata(&output) {
        // Atomic replacement must still honor the destination's write permissions.
        fs::OpenOptions::new()
            .write(true)
            .open(&output)
            .map_err(|e| format!("Error saving file {}: {e}", output.display()))?;
        staging
            .as_file()
            .set_permissions(metadata.permissions())
            .map_err(|e| e.to_string())?;
    }
    let mut out = BufWriter::new(staging.as_file_mut());
    let result = (|| -> io::Result<()> {
        out.write_all(b"<?xml version='1.0' encoding='utf-8'?>\n")?;
        let wrapper = Element {
            tag: options.root.clone(),
            ..Element::default()
        };
        // Write the opening wrapper once; each input tree is released immediately.
        let mut head = Vec::new();
        serialize(&wrapper, &ns, &mut head, true)?;
        head.truncate(head.len() - 3);
        out.write_all(&head)?;
        out.write_all(b">")?;
        let mut seen = HashSet::new();
        let mut count = 0;
        for (path, fragment) in valid {
            if !options.deduplicate {
                let mut serializer = StreamingSerializer {
                    out: &mut out,
                    ns: &ns,
                    flatten: options.flatten,
                    fragment,
                    frames: vec![],
                    root_seen: false,
                    count: 0,
                };
                let mut emit = |notice| serializer.accept(notice).map_err(|e| e.to_string());
                parse_file_with_handlers(File::open(path)?, None, Some(&mut emit))
                    .map_err(io::Error::other)?;
                count += serializer.count;
                continue;
            }
            let mut emit = |node: Element| {
                if options.deduplicate && !seen.insert(element_key(&node)) {
                    if options.verbose {
                        log(
                            "DEBUG",
                            &format!("Skipping duplicate element: {}", node.tag),
                        );
                    }
                    return Ok(());
                }
                serialize(&node, &ns, &mut out, false).map_err(|e| e.to_string())?;
                count += 1;
                Ok(())
            };
            let file = File::open(path)?;
            parse_file_with_sink(file, Some((&mut emit, options.flatten)))
                .map_err(io::Error::other)?;
        }
        if count == 0 {
            out.flush()?;
            // Empty output uses ElementTree's short-element spelling as well.
            out.get_mut().set_len(0)?;
            use std::io::{Seek, SeekFrom};
            out.get_mut().seek(SeekFrom::Start(0))?;
            let mut file = out.get_mut();
            file.write_all(b"<?xml version='1.0' encoding='utf-8'?>\n")?;
            serialize(&wrapper, &ns, &mut file, true)?;
        } else {
            write!(out, "</{}>", qualified(&options.root, &ns))?;
            out.flush()?;
        }
        Ok(())
    })();
    result.map_err(|e| format!("Error saving file {}: {e}", output.display()))?;
    drop(out);
    staging
        .persist(&output)
        .map_err(|e| format!("Error saving file {}: {e}", output.display()))?;
    log(
        "INFO",
        &format!(
            "Combined XML file saved: {}",
            output.display().to_string().trim_start_matches(r"\\?\")
        ),
    );
    Ok(())
}

#[cfg(test)]
#[path = "../tests/reader/mod.rs"]
mod reader_tests;
