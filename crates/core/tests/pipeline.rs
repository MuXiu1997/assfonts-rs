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
    .process_with_policy("input", MissingGlyphPolicy::Error)
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
    .process_with_policy("input", MissingGlyphPolicy::Error)
    .unwrap();
    assert_eq!(result.attachments.len(), 1);
    assert_eq!(*backend.0.lock().unwrap(), vec![BTreeSet::new()]);
}

#[test]
fn warning_plan_order_survives_grouping_and_unimplemented_plugins_reject() {
    assert_eq!(MissingGlyphPolicy::default(), MissingGlyphPolicy::Warn);
    let backend = Backend::default();
    let processor = Processor {
        codec: &Codec,
        resolver: &Resolver,
        subsetter: &backend,
    };
    assert!(processor
        .process_with_policy("input", MissingGlyphPolicy::Warn)
        .is_err());
    struct Planned;
    impl FontResolver for Planned {
        fn resolve(&self, _: &FontRequest, _: &BTreeSet<char>) -> Result<FontFace> {
            unreachable!()
        }
        fn plan(&self, usage: &FontUsage, _: MissingGlyphPolicy) -> Result<FontPlan> {
            let request = usage.keys().next().unwrap().clone();
            Ok(FontPlan {
                fonts: [("later", 9, 'B'), ("first", 2, 'A'), ("first", 2, 'C')]
                    .into_iter()
                    .map(|(source, order, ch)| PlannedFont {
                        face: FontFace {
                            source: source.into(),
                            data: Arc::from(source.as_bytes()),
                            index: 0,
                        },
                        characters: [ch].into(),
                        request: request.clone(),
                        order,
                    })
                    .collect(),
                warnings: Vec::new(),
            })
        }
    }
    struct WarnBackend;
    impl Subsetter for WarnBackend {
        fn name(&self) -> &str {
            "warning-test"
        }
        fn subset(&self, _: &FontFace, _: &BTreeSet<char>) -> Result<Vec<u8>> {
            unreachable!()
        }
        fn subset_with_policy(
            &self,
            face: &FontFace,
            _: &BTreeSet<char>,
            policy: MissingGlyphPolicy,
        ) -> Result<Vec<u8>> {
            assert_eq!(policy, MissingGlyphPolicy::Warn);
            Ok(face.data.to_vec())
        }
    }
    assert!(Processor {
        codec: &Codec,
        resolver: &Planned,
        subsetter: &backend
    }
    .process_with_policy("input", MissingGlyphPolicy::Warn)
    .is_err());
    let result = Processor {
        codec: &Codec,
        resolver: &Planned,
        subsetter: &WarnBackend,
    }
    .process("input")
    .unwrap();
    assert_eq!(
        result
            .report
            .fonts
            .iter()
            .map(|f| f.source.as_str())
            .collect::<Vec<_>>(),
        ["first", "later"]
    );
    assert_eq!(result.report.fonts[0].characters, "AC");
    assert_eq!(result.attachments.len(), 2);
    assert_eq!(result.report.missing_glyph_policy, MissingGlyphPolicy::Warn);
}
