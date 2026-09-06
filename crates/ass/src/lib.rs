//! ASS analysis adapter. Preserves source text; deliberately rejects unsafe guesses.
#![forbid(unsafe_code)]

mod attachment;
use ass_core::{analysis::events::parse_override_block, parser::ast::Section, Script};
use assfonts_core::{Attachment, Error, FontRequest, FontUsage, Result, SubtitleCodec};
use std::{borrow::Cow, collections::BTreeMap};

#[derive(Default)]
pub struct AssCodec;

fn invalid(message: impl Into<String>) -> Error {
    Error::Subtitle(message.into())
}
fn integer(value: &str) -> Result<i32> {
    value
        .trim()
        .parse()
        .map_err(|_| invalid(format!("expected integer, got {value:?}")))
}

fn headers(text: &str) -> Result<()> {
    let mut styles = 0;
    let mut events = 0;
    for line in text.trim_start_matches('\u{feff}').lines().map(str::trim) {
        match line.to_ascii_lowercase().as_str() {
            "[fonts]" => {
                return Err(Error::Unsupported(
                    "existing [Fonts] attachments; use the original unembedded ASS".into(),
                ))
            }
            "[v4 styles]" | "[v4++ styles]" => {
                return Err(Error::Unsupported("only ASS v4+ is supported".into()))
            }
            "[v4+ styles]" => styles += 1,
            "[events]" => events += 1,
            _ => (),
        }
    }
    if styles != 1 || events != 1 {
        return Err(invalid(
            "expected exactly one [V4+ Styles] and [Events] section",
        ));
    }
    if text.contains('\0') {
        return Err(invalid("NUL in subtitle"));
    }
    Ok(())
}

// Editor metadata is not rendered. Hide only these known sections from the
// syntax parser; keep the original bytes for embedding and all other diagnostics.
// Comment out each line in place to retain diagnostic line/byte positions.
fn analysis_text(text: &str) -> Cow<'_, str> {
    let mut masked: Option<Vec<u8>> = None;
    let mut metadata = false;
    let mut offset = 0;
    for line in text.split_inclusive('\n') {
        let trimmed = line.trim();
        if trimmed.starts_with('[') {
            metadata = [
                "[Aegisub Project Garbage]",
                "[Aegisub Project]",
                "[Aegisub Extradata]",
            ]
            .iter()
            .any(|header| trimmed.eq_ignore_ascii_case(header));
        }
        if metadata && !trimmed.is_empty() {
            let bytes = masked.get_or_insert_with(|| text.as_bytes().to_vec());
            for byte in &mut bytes[offset..offset + line.len()] {
                if !matches!(*byte, b'\r' | b'\n') {
                    *byte = b' ';
                }
            }
            bytes[offset] = b';';
        }
        offset += line.len();
    }
    match masked {
        Some(bytes) => {
            Cow::Owned(String::from_utf8(bytes).expect("whole lines replaced with ASCII"))
        }
        None => Cow::Borrowed(text),
    }
}

fn ignored_tag(name: &str) -> bool {
    matches!(
        name,
        "fs" | "fscx"
            | "fscy"
            | "fsp"
            | "fr"
            | "frx"
            | "fry"
            | "frz"
            | "fax"
            | "fay"
            | "c"
            | "1c"
            | "2c"
            | "3c"
            | "4c"
            | "alpha"
            | "1a"
            | "2a"
            | "3a"
            | "4a"
            | "bord"
            | "xbord"
            | "ybord"
            | "shad"
            | "xshad"
            | "yshad"
            | "blur"
            | "be"
            | "an"
            | "a"
            | "pos"
            | "move"
            | "org"
            | "clip"
            | "iclip"
            | "fad"
            | "fade"
            | "k"
            | "K"
            | "kf"
            | "ko"
            | "kt"
            | "u"
            | "s"
            | "pbo"
    )
}

// A transform can change font selection over time; do not silently miss its faces.
fn check_transform(args: &str) -> Result<()> {
    if !args.starts_with('(') || !args.ends_with(')') {
        return Err(invalid("malformed transform"));
    }
    let Some(start) = args.find('\\') else {
        return Err(invalid("transform without tags"));
    };
    let mut tags = Vec::new();
    let mut diagnostics = Vec::new();
    parse_override_block(&args[start..args.len() - 1], 0, &mut tags, &mut diagnostics);
    if !diagnostics.is_empty() {
        return Err(invalid(format!("transform diagnostics: {diagnostics:?}")));
    }
    for tag in tags {
        if !ignored_tag(tag.name()) {
            return Err(Error::Unsupported(format!(
                "font/state-changing or unknown transform tag \\{}",
                tag.name()
            )));
        }
    }
    Ok(())
}

impl SubtitleCodec for AssCodec {
    fn analyze(&self, subtitle: &str) -> Result<FontUsage> {
        headers(subtitle)?;
        let text = subtitle.trim_start_matches('\u{feff}');
        let parser_text = analysis_text(text);
        let script = Script::parse(&parser_text).map_err(|e| invalid(e.to_string()))?;
        if !script.issues().is_empty() {
            return Err(invalid(format!(
                "parser diagnostics: {:?}",
                script.issues()
            )));
        }
        let mut styles = BTreeMap::new();
        let mut wrap_style = 0;
        // Restrict WrapStyle lookup to the actual Script Info section.
        let mut in_info = false;
        for line in text.lines().map(str::trim) {
            if line.starts_with('[') {
                in_info = line.eq_ignore_ascii_case("[Script Info]");
            }
            if in_info {
                if let Some((key, value)) = line.split_once(':') {
                    if key.eq_ignore_ascii_case("WrapStyle") {
                        wrap_style = integer(value)?;
                    }
                }
            }
        }
        if !(0..=3).contains(&wrap_style) {
            return Err(invalid("WrapStyle must be 0..3"));
        }
        for section in script.sections() {
            if let Section::Styles(items) = section {
                for s in items {
                    let request = FontRequest {
                        family: s.fontname.trim().into(),
                        weight: if integer(s.bold)? != 0 { 700 } else { 400 },
                        italic: integer(s.italic)? != 0,
                    };
                    if request.family.is_empty() {
                        return Err(invalid("empty font name"));
                    }
                    if request.family.starts_with('@') {
                        return Err(Error::Unsupported("vertical @font names".into()));
                    }
                    if styles.insert(s.name, request).is_some() {
                        return Err(invalid(format!("duplicate style {}", s.name)));
                    }
                }
            }
        }
        let mut usage = FontUsage::new();
        for section in script.sections() {
            let Section::Events(events) = section else {
                continue;
            };
            for (event_index, event) in events.iter().enumerate() {
                if !event.is_dialogue() {
                    continue;
                }
                integer(event.layer)
                    .map_err(|e| invalid(format!("dialogue {} layer: {e}", event_index + 1)))?;
                let result =
                    analyze_event(event.text, event.style, &styles, wrap_style, &mut usage);
                result.map_err(|e| invalid(format!("dialogue {}: {e}", event_index + 1)))?;
            }
        }
        Ok(usage)
    }

    fn embed(&self, subtitle: &str, fonts: &[Attachment]) -> Result<String> {
        headers(subtitle)?;
        attachment::embed(subtitle, fonts)
    }
}

fn analyze_event(
    text: &str,
    style: &str,
    styles: &BTreeMap<&str, FontRequest>,
    wrap_style: i32,
    usage: &mut FontUsage,
) -> Result<()> {
    let base = styles
        .get(style)
        .ok_or_else(|| invalid(format!("unknown style {style:?}")))?;
    let mut active_style = base;
    let mut current = base.clone();
    let mut drawing = false;
    let mut wrap = wrap_style;
    let mut pos = 0;
    while pos < text.len() {
        let rest = &text[pos..];
        if rest.starts_with('{') {
            let end = rest
                .find('}')
                .ok_or_else(|| invalid("unclosed override block"))?;
            let mut tags = Vec::new();
            let mut diagnostics = Vec::new();
            parse_override_block(&rest[1..end], pos + 1, &mut tags, &mut diagnostics);
            if !diagnostics.is_empty() {
                return Err(invalid(format!("tag diagnostics: {diagnostics:?}")));
            }
            for tag in tags {
                let arg = tag.args().trim();
                match tag.name() {
                    "fn" => {
                        current.family = if arg.is_empty() || arg == "0" {
                            active_style.family.clone()
                        } else {
                            arg.into()
                        };
                        if current.family.starts_with('@') {
                            return Err(Error::Unsupported("vertical @font names".into()));
                        }
                    }
                    "b" => {
                        current.weight = if arg.is_empty() {
                            active_style.weight
                        } else {
                            match integer(arg)? {
                                0 => 400,
                                1 => 700,
                                n @ 100..=900 => n as u16,
                                _ => return Err(Error::Unsupported(format!("bold weight {arg}"))),
                            }
                        }
                    }
                    "i" => {
                        current.italic = if arg.is_empty() {
                            active_style.italic
                        } else {
                            match integer(arg)? {
                                0 => false,
                                1 => true,
                                _ => return Err(invalid("italic must be 0 or 1")),
                            }
                        }
                    }
                    "r" => {
                        active_style = if arg.is_empty() {
                            base
                        } else {
                            styles
                                .get(arg)
                                .ok_or_else(|| invalid(format!("unknown reset style {arg:?}")))?
                        };
                        current = active_style.clone();
                    }
                    "p" => {
                        drawing = !arg.is_empty() && integer(arg)? > 0;
                    }
                    "q" => {
                        wrap = if arg.is_empty() {
                            wrap_style
                        } else {
                            integer(arg)?
                        };
                        if !(0..=3).contains(&wrap) {
                            return Err(invalid("wrap mode must be 0..3"));
                        }
                    }
                    "t" => check_transform(arg)?,
                    name if ignored_tag(name) => (),
                    name => return Err(Error::Unsupported(format!("override tag \\{name}"))),
                }
            }
            pos += end + 1;
        } else {
            let end = rest.find('{').unwrap_or(rest.len());
            if !drawing {
                let mut chars = rest[..end].chars().peekable();
                while let Some(mut ch) = chars.next() {
                    if ch == '\\' {
                        match chars.peek() {
                            Some('N') => {
                                chars.next();
                                continue;
                            }
                            Some('n') => {
                                chars.next();
                                if wrap == 2 {
                                    continue;
                                }
                                ch = ' ';
                            }
                            Some('h') => {
                                chars.next();
                                ch = '\u{a0}';
                            }
                            _ => (), // libass renders unknown escapes as literal text.
                        }
                    }
                    if ch.is_control() {
                        return Err(Error::Unsupported(format!("control character {ch:?}")));
                    }
                    usage.entry(current.clone()).or_default().insert(ch);
                }
            }
            pos += end;
        }
    }
    Ok(())
}
