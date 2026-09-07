// Preserve the AAT layout path when subsetting legacy mort fonts. HarfBuzz
// drops mort by default, which can unexpectedly activate a different GSUB
// program even for horizontal text when mort only contains vertical rules.
// We support non-contextual substitutions, close their output glyphs and
// serialize compact, renumbered lookup tables. Unsupported state machines
// fail rather than silently changing shaping or embedding a complete font.
#include <algorithm>
#include <utility>
#include <vector>

namespace af_mort {
using bytes = std::vector<char>;
struct view {
    const char* data;
    size_t size;
    bool has(size_t off, size_t n) const { return off <= size && n <= size - off; }
    uint16_t u16(size_t off) const {
        return (uint16_t(uint8_t(data[off])) << 8) | uint8_t(data[off + 1]);
    }
    uint32_t u32(size_t off) const { return (uint32_t(u16(off)) << 16) | u16(off + 2); }
};
static void put16(bytes& out, uint16_t v) { out.push_back(v >> 8); out.push_back(v & 255); }
static void put32(bytes& out, uint32_t v) { put16(out, v >> 16); put16(out, v & 65535); }
struct subtable {
    uint16_t coverage;
    uint32_t flags;
    std::vector<std::pair<uint16_t, uint16_t>> substitutions;
};
struct chain {
    uint32_t flags;
    uint16_t feature_count;
    bytes features;
    std::vector<subtable> subtables;
};
struct table {
    std::vector<chain> chains;
    bool present = false;
};

static bool lookup(view v, uint32_t glyph_count, subtable& sub) {
    if (!v.has(0, 2)) return false;
    auto add = [&](uint32_t from, uint32_t to) {
        if (from >= glyph_count || (to >= glyph_count && to != 65535)) return false;
        if (from != to) sub.substitutions.emplace_back(from, to);
        return true;
    };
    uint16_t format = v.u16(0);
    if (format == 0) {
        if (!v.has(2, size_t(glyph_count) * 2)) return false;
        for (uint32_t g = 0; g < glyph_count; g++)
            if (!add(g, v.u16(2 + g * 2))) return false;
    } else if (format == 2 || format == 4 || format == 6) {
        if (!v.has(0, 12)) return false;
        uint16_t unit = v.u16(2), count = v.u16(4);
        if (unit != (format == 6 ? 4 : 6) || !v.has(12, size_t(unit) * count)) return false;
        uint32_t previous = 0; bool any = false;
        for (uint32_t i = 0; i < count; i++) {
            size_t at = 12 + i * unit;
            uint32_t last = v.u16(at), first = format == 6 ? last : v.u16(at + 2);
            uint32_t value = v.u16(at + unit - 2);
            if (first == 65535 && last == 65535) continue; // optional search sentinel
            if (first > last || last >= glyph_count || (any && first <= previous)) return false;
            previous = last; any = true;
            for (uint32_t g = first; g <= last; g++) {
                size_t off = value + (g - first) * 2;
                if (format == 4 && !v.has(off, 2)) return false;
                if (!add(g, format == 4 ? v.u16(off) : value)) return false;
            }
        }
    } else if (format == 8 || format == 10) {
        size_t header = format == 8 ? 6 : 8;
        if (!v.has(0, header)) return false;
        uint32_t unit = format == 8 ? 2 : v.u16(2);
        uint32_t first = v.u16(header - 4), count = v.u16(header - 2);
        if ((unit != 1 && unit != 2 && unit != 4) || first + count > glyph_count ||
            !v.has(header, size_t(count) * unit)) return false;
        for (uint32_t i = 0; i < count; i++) {
            size_t off = header + i * unit;
            uint32_t value = unit == 1 ? uint8_t(v.data[off]) : unit == 2 ? v.u16(off) : v.u32(off);
            if (!add(first + i, value)) return false;
        }
    } else return false;
    return true;
}

static bool read(hb_face_t* face, table& out) {
    owned<hb_blob_t, hb_blob_destroy> blob(hb_face_reference_table(face, HB_TAG('m','o','r','t')), hb_blob_destroy);
    unsigned length = 0;
    const char* data = hb_blob_get_data(blob.get(), &length);
    if (!length) return true;
    out.present = true;
    view v{data, length};
    if (!v.has(0, 8) || v.u32(0) != 0x00010000) return false;
    uint32_t count = v.u32(4), glyphs = hb_face_get_glyph_count(face);
    if (count > (length - 8) / 12 || glyphs > 65535) return false;
    size_t at = 8;
    for (uint32_t i = 0; i < count; i++) {
        if (!v.has(at, 12)) return false;
        uint32_t size = v.u32(at + 4);
        if (size < 12 || !v.has(at, size)) return false;
        view cv{v.data + at, size};
        chain ch{cv.u32(0), cv.u16(8), {}, {}};
        uint16_t subs = cv.u16(10);
        size_t off = 12 + size_t(ch.feature_count) * 12;
        if (!cv.has(12, off - 12)) return false;
        ch.features.assign(cv.data + 12, cv.data + off);
        for (uint32_t j = 0; j < subs; j++) {
            if (!cv.has(off, 8)) return false;
            uint16_t len = cv.u16(off), coverage = cv.u16(off + 2);
            if (len < 10 || !cv.has(off, len) || (coverage & 7) != 4) return false;
            subtable sub{coverage, cv.u32(off + 4), {}};
            if (!lookup({cv.data + off + 8, size_t(len - 8)}, glyphs, sub)) return false;
            ch.subtables.push_back(std::move(sub)); off += len;
        }
        out.chains.push_back(std::move(ch)); at += size;
    }
    return true;
}

// Repeat after HB's GSUB/component closure: those glyphs can themselves be
// inputs to mort. A finite source glyph set bounds this fixed-point loop.
static bool close(const table& mort, const hb_map_t* mapping, hb_set_t* glyphs) {
    bool changed = false;
    for (const auto& ch : mort.chains) for (const auto& sub : ch.subtables)
        for (auto [from, to] : sub.substitutions)
            if (to != 65535 && hb_map_has(mapping, from) && !hb_map_has(mapping, to) && !hb_set_has(glyphs, to)) {
                hb_set_add(glyphs, to); changed = true;
            }
    return changed;
}

static bool write(const table& mort, const hb_map_t* mapping, bytes& out) {
    put32(out, 0x00010000); put32(out, mort.chains.size());
    for (const auto& ch : mort.chains) {
        bytes body = ch.features;
        for (const auto& sub : ch.subtables) {
            std::vector<std::pair<uint16_t, uint16_t>> pairs;
            for (auto [from, to] : sub.substitutions) {
                if (!hb_map_has(mapping, from)) continue;
                if (to != 65535 && !hb_map_has(mapping, to)) return false;
                uint32_t a = hb_map_get(mapping, from), b = to == 65535 ? 65535 : hb_map_get(mapping, to);
                if (a >= 65535 || b > 65535) return false;
                pairs.emplace_back(a, b);
            }
            std::sort(pairs.begin(), pairs.end());
            if (pairs.size() > (65535 - 20) / 4) return false;
            put16(body, 20 + pairs.size() * 4); put16(body, sub.coverage); put32(body, sub.flags);
            // AAT lookup format 6, sorted (glyph, replacement) records.
            uint16_t power = 0, selector = 0;
            if (!pairs.empty()) { power = 1; while (size_t(power) * 2 <= pairs.size()) { power *= 2; selector++; } }
            put16(body, 6); put16(body, 4); put16(body, pairs.size());
            put16(body, power * 4); put16(body, selector); put16(body, pairs.size() * 4 - power * 4);
            for (auto [from, to] : pairs) { put16(body, from); put16(body, to); }
        }
        if (body.size() > UINT32_MAX - 12) return false;
        put32(out, ch.flags); put32(out, body.size() + 12); put16(out, ch.feature_count); put16(out, ch.subtables.size());
        out.insert(out.end(), body.begin(), body.end());
    }
    return true;
}

static hb_blob_t* attach(hb_face_t* subset, const bytes& mort) {
    owned<hb_face_t, hb_face_destroy> builder(hb_face_builder_create(), hb_face_destroy);
    unsigned count = hb_face_get_table_tags(subset, 0, nullptr, nullptr);
    std::vector<hb_tag_t> tags(count);
    hb_face_get_table_tags(subset, 0, &count, tags.data());
    for (hb_tag_t tag : tags) {
        if (tag == HB_TAG('m','o','r','t')) continue;
        owned<hb_blob_t, hb_blob_destroy> blob(hb_face_reference_table(subset, tag), hb_blob_destroy);
        if (!hb_face_builder_add_table(builder.get(), tag, blob.get())) return nullptr;
    }
    owned<hb_blob_t, hb_blob_destroy> blob(hb_blob_create(mort.data(), mort.size(), HB_MEMORY_MODE_DUPLICATE, nullptr, nullptr), hb_blob_destroy);
    if (hb_blob_get_length(blob.get()) != mort.size() || !hb_face_builder_add_table(builder.get(), HB_TAG('m','o','r','t'), blob.get())) return nullptr;
    return hb_face_reference_blob(builder.get());
}
} // namespace af_mort
