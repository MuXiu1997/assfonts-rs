use assfonts_core::*;
use std::{
    collections::BTreeSet,
    sync::{Arc, Mutex},
};

struct Codec;
impl SubtitleCodec for Codec {
    fn analyze(&self, _: &str) -> Result<FontUsage> {
        Ok([
            (
                FontRequest {
                    family: "Alias A".into(),
                    weight: 400,
                    italic: false,
                },
                ['A'].into(),
            ),
            (
                FontRequest {
                    family: "Alias B".into(),
                    weight: 700,
                    italic: false,
                },
                ['B'].into(),
            ),
        ]
        .into())
    }
    fn embed(&self, text: &str, fonts: &[Attachment]) -> Result<String> {
        Ok(format!("{text}:{}", fonts.len()))
    }
}
struct Resolver;
impl FontResolver for Resolver {
    fn resolve(&self, _: &FontRequest, _: &BTreeSet<char>) -> Result<FontFace> {
        Ok(FontFace {
            source: "in-memory".into(),
            data: Arc::from(&b"same face"[..]),
            index: 2,
        })
    }
}
#[derive(Default)]
struct Backend(Mutex<Vec<BTreeSet<char>>>);
impl Subsetter for Backend {
    fn name(&self) -> &str {
        "test-backend"
    }
    fn subset(&self, _: &FontFace, chars: &BTreeSet<char>) -> Result<Vec<u8>> {
        self.0.lock().unwrap().push(chars.clone());
        Ok(b"OTTOtest".to_vec())
    }
}
#[test]
fn plugins_work_without_native_dependencies_and_union_face_usage() {
    let backend = Backend::default();
    let result = Processor {
        codec: &Codec,
        resolver: &Resolver,
        subsetter: &backend,
    }
    .process("input")
    .unwrap();
    assert_eq!(*backend.0.lock().unwrap(), vec![['A', 'B'].into()]);
    assert_eq!(result.subtitle, "input:1");
    assert_eq!(result.report.fonts[0].requests.len(), 2);
    assert_eq!(result.report.fonts[0].face_index, 2);
    assert_eq!(result.report.backend, "test-backend");
}

#[test]
fn an_empty_character_set_is_still_a_font_dependency() {
    struct Drawing;
    impl SubtitleCodec for Drawing {
        fn analyze(&self, _: &str) -> Result<FontUsage> {
            Ok([(
                FontRequest {
                    family: "Drawing".into(),
                    weight: 700,
                    italic: false,
                },
                BTreeSet::new(),
            )]
            .into())
        }
        fn embed(&self, text: &str, fonts: &[Attachment]) -> Result<String> {
            Codec.embed(text, fonts)
        }
    }
    let backend = Backend::default();
    let result = Processor {
        codec: &Drawing,
        resolver: &Resolver,
        subsetter: &backend,
    }
    .process("input")
    .unwrap();
    assert_eq!(result.attachments.len(), 1);
    assert_eq!(*backend.0.lock().unwrap(), vec![BTreeSet::new()]);
}
