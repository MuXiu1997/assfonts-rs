// The small C ABI is private to this crate. Native resources never cross it
// except an owned output blob; all destruction uses the allocating library.
#include "hb.h"
#include "hb-subset.h"
// Use the exact sanitizer used by this pinned HarfBuzz renderer. A rejected
// optional BASE table is invisible to shaping, but otherwise aborts subsetting.
#include "hb-ot-var-common.hh"
#include "hb-ot-layout-base-table.hh"
#include <cstdint>
#include <initializer_list>
#include <memory>

template <typename T, void (*Destroy)(T*)>
using owned = std::unique_ptr<T, decltype(Destroy)>;

#include "mort.hh"

// Shaping normalizes Unicode before glyph lookup. Keep source cmap entries for
// canonical compositions/decompositions as well as the literal ASS characters.
// This operates on font coverage only; the subtitle bytes are never normalized.
static void add_decomposition(hb_unicode_funcs_t* unicode, hb_set_t* set,
                              hb_codepoint_t cp) {
    hb_set_add(set, cp);
    hb_codepoint_t a, b;
    if (hb_unicode_decompose(unicode, cp, &a, &b)) {
        add_decomposition(unicode, set, a);
        if (b) add_decomposition(unicode, set, b);
    }
}

static bool decomposition_available(hb_unicode_funcs_t* unicode,
                                    const hb_set_t* set, hb_codepoint_t cp) {
    if (hb_set_has(set, cp)) return true;
    hb_codepoint_t a, b;
    return hb_unicode_decompose(unicode, cp, &a, &b) &&
        decomposition_available(unicode, set, a) &&
        (!b || decomposition_available(unicode, set, b));
}

static bool close_normalization(hb_face_t* face, hb_set_t* chars) {
    owned<hb_set_t, hb_set_destroy> decomposed(hb_set_create(), hb_set_destroy);
    owned<hb_set_t, hb_set_destroy> available(hb_set_create(), hb_set_destroy);
    auto* unicode = hb_unicode_funcs_get_default();
    hb_codepoint_t cp = HB_SET_VALUE_INVALID;
    while (hb_set_next(chars, &cp)) add_decomposition(unicode, decomposed.get(), cp);
    hb_face_collect_unicodes(face, available.get());
    cp = HB_SET_VALUE_INVALID;
    while (hb_set_next(available.get(), &cp)) {
        if (decomposition_available(unicode, decomposed.get(), cp)) hb_set_add(chars, cp);
    }
    return hb_set_allocation_successful(decomposed.get()) &&
        hb_set_allocation_successful(available.get()) && hb_set_allocation_successful(chars);
}

extern "C" {
hb_blob_t* af_subset(const char* bytes, uint32_t length, uint32_t index,
                     const uint32_t* unicodes, uint32_t count, uint32_t preserve_notdef) {
    owned<hb_blob_t, hb_blob_destroy> blob(
        hb_blob_create(bytes, length, HB_MEMORY_MODE_READONLY, nullptr, nullptr), hb_blob_destroy);
    if (hb_blob_get_length(blob.get()) != length) return nullptr;
    owned<hb_face_t, hb_face_destroy> face(hb_face_create(blob.get(), index), hb_face_destroy);
    if (!hb_face_get_glyph_count(face.get())) return nullptr;
    owned<hb_subset_input_t, hb_subset_input_destroy> input(hb_subset_input_create_or_fail(), hb_subset_input_destroy);
    if (!input) return nullptr;
    {
        owned<hb_blob_t, hb_blob_destroy> raw(hb_face_reference_table(face.get(), HB_TAG('B','A','S','E')), hb_blob_destroy);
        if (hb_blob_get_length(raw.get())) {
            owned<hb_blob_t, hb_blob_destroy> sanitized(hb_sanitize_context_t().reference_table<OT::BASE>(face.get()), hb_blob_destroy);
            if (!hb_blob_get_length(sanitized.get()))
                hb_set_add(hb_subset_input_set(input.get(), HB_SUBSET_SETS_DROP_TABLE_TAG), HB_TAG('B','A','S','E'));
        }
    }
    hb_set_t* chars = hb_subset_input_unicode_set(input.get());
    hb_set_add_sorted_array(chars, unicodes, count);
    if (!hb_set_allocation_successful(chars)) return nullptr;
    if (!close_normalization(face.get(), chars)) return nullptr;
    // Optional renderer support characters, not mandatory source coverage.
    // Include only glyphs actually supplied by this face. ASCII includes digits,
    // case variants and punctuation; Latin-1 adds NBSP; fullwidth ASCII and
    // ideographic space preserve common alternate-width usage and fallbacks.
    owned<hb_set_t, hb_set_destroy> available(hb_set_create(), hb_set_destroy);
    hb_face_collect_unicodes(face.get(), available.get());
    for (auto range : {std::pair<uint32_t,uint32_t>{0x20, 0xff}, {0xff01, 0xff5e}, {0x3000, 0x3000}})
        for (uint32_t cp = range.first; cp <= range.second; cp++)
            if (hb_set_has(available.get(), cp)) hb_set_add(chars, cp);
    if (!hb_set_allocation_successful(available.get()) || !hb_set_allocation_successful(chars)) return nullptr;
    // Keep localized/legacy names and all layout features; retain default
    // glyph closure, bidi closure and hinting for renderer compatibility.
    hb_subset_input_set_flags(input.get(), HB_SUBSET_FLAGS_NAME_LEGACY |
        (preserve_notdef ? HB_SUBSET_FLAGS_NOTDEF_OUTLINE : 0));
    for (auto type : {HB_SUBSET_SETS_NAME_ID, HB_SUBSET_SETS_NAME_LANG_ID, HB_SUBSET_SETS_LAYOUT_FEATURE_TAG}) {
        hb_set_t* set = hb_subset_input_set(input.get(), type);
        hb_set_clear(set);
        hb_set_invert(set);
        if (!hb_set_allocation_successful(set)) return nullptr;
    }
    af_mort::table mort;
    if (!af_mort::read(face.get(), mort)) return nullptr;
    owned<hb_subset_plan_t, hb_subset_plan_destroy> plan(nullptr, hb_subset_plan_destroy);
    auto* glyphs = hb_subset_input_glyph_set(input.get());
    do {
        plan.reset(hb_subset_plan_create_or_fail(face.get(), input.get()));
        if (!plan || !hb_set_allocation_successful(glyphs)) return nullptr;
    } while (af_mort::close(mort, hb_subset_plan_old_to_new_glyph_mapping(plan.get()), glyphs));
    owned<hb_face_t, hb_face_destroy> subset(hb_subset_plan_execute_or_fail(plan.get()), hb_face_destroy);
    if (!subset) return nullptr;
    if (mort.present) {
        af_mort::bytes table;
        if (!af_mort::write(mort, hb_subset_plan_old_to_new_glyph_mapping(plan.get()), table)) return nullptr;
        return af_mort::attach(subset.get(), table);
    }
    hb_blob_t* output = hb_face_reference_blob(subset.get());
    if (!hb_blob_get_length(output)) {hb_blob_destroy(output); return nullptr;}
    return output;
}

const char* af_blob_data(hb_blob_t* blob, uint32_t* length) {return hb_blob_get_data(blob,length);}
void af_blob_destroy(hb_blob_t* blob) {hb_blob_destroy(blob);}
const char* af_version() {return hb_version_string();}
}
