use bohmian_code_review_extract::{ExtractionConfig, Extractor};

#[test]
/// Confirms native recovery matches the frozen C/C++ header oracle graph shape.
///
/// # Panics
///
/// Panics when native configuration or extraction fails, or when the recovered
/// graph shape differs from the frozen oracle.
fn probe_score_common_header() {
    let path = "libs/simdvec/native/src/vec/headers/score_common.h";
    let source = include_str!("fixtures/score_common.h");
    let result = Extractor::new(ExtractionConfig::all_native())
        .expect("native config")
        .extract_from_source(path, source)
        .expect("native recovery");
    assert_eq!(result.nodes.len(), 10);
    assert_eq!(result.edges.len(), 12);
    assert_eq!(result.references.len(), 6);
    assert_eq!(result.references[0].reference_name, "vec.h");
    assert_eq!(result.references[1].reference_name, "f32_t");
    assert_eq!(result.references[4].reference_name, "__builtin_fmaxf");
    assert_eq!(result.references[0].line, 5);
    assert_eq!(result.references[1].line, 8);
    assert_eq!(result.references[2].line, 9);
    assert_eq!(result.references[3].line, 11);
    assert_eq!(result.references[4].line, 47);
    assert_eq!(result.references[5].line, 107);
}
