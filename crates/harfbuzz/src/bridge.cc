// The small C ABI is private to this crate. Native resources never cross it
// except an owned output blob; all destruction uses the allocating library.
#include "hb.h"
#include "hb-subset.h"
#include <cstdint>
#include <memory>

template <typename T, void (*Destroy)(T*)>
using owned = std::unique_ptr<T, decltype(Destroy)>;

extern "C" {
hb_blob_t* af_subset(const char* bytes, uint32_t length, uint32_t index,
                     const uint32_t* unicodes, uint32_t count) {
    owned<hb_blob_t, hb_blob_destroy> blob(
        hb_blob_create(bytes, length, HB_MEMORY_MODE_READONLY, nullptr, nullptr), hb_blob_destroy);
    if (hb_blob_get_length(blob.get()) != length) return nullptr;
    owned<hb_face_t, hb_face_destroy> face(hb_face_create(blob.get(), index), hb_face_destroy);
    if (!hb_face_get_glyph_count(face.get())) return nullptr;
    owned<hb_subset_input_t, hb_subset_input_destroy> input(hb_subset_input_create_or_fail(), hb_subset_input_destroy);
    if (!input) return nullptr;
    hb_set_t* chars = hb_subset_input_unicode_set(input.get());
    hb_set_add_sorted_array(chars, unicodes, count);
    if (!hb_set_allocation_successful(chars)) return nullptr;
    // Keep localized/legacy names and all layout features; retain default
    // glyph closure, bidi closure and hinting for renderer compatibility.
    hb_subset_input_set_flags(input.get(), HB_SUBSET_FLAGS_NAME_LEGACY);
    for (auto type : {HB_SUBSET_SETS_NAME_ID, HB_SUBSET_SETS_NAME_LANG_ID, HB_SUBSET_SETS_LAYOUT_FEATURE_TAG}) {
        hb_set_t* set = hb_subset_input_set(input.get(), type);
        hb_set_clear(set);
        hb_set_invert(set);
        if (!hb_set_allocation_successful(set)) return nullptr;
    }
    owned<hb_face_t, hb_face_destroy> subset(hb_subset_or_fail(face.get(),input.get()), hb_face_destroy);
    if (!subset) return nullptr;
    hb_blob_t* output = hb_face_reference_blob(subset.get());
    if (!hb_blob_get_length(output)) {hb_blob_destroy(output); return nullptr;}
    return output;
}

const char* af_blob_data(hb_blob_t* blob, uint32_t* length) {return hb_blob_get_data(blob,length);}
void af_blob_destroy(hb_blob_t* blob) {hb_blob_destroy(blob);}
const char* af_version() {return hb_version_string();}
}
