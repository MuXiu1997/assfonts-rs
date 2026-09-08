//! Font-analysis view of libass 0.17.5 override token boundaries.
//! Keep this order: libass matches prefixes, not complete tag names.

const NAMES: &[&str] = &[
    "xbord", "ybord", "xshad", "yshad", "fax", "fay", "iclip", "blur", "fscx", "fscy", "fsc",
    "fsp", "fs", "bord", "move", "frx", "fry", "frz", "fr", "fn", "alpha", "an", "a", "pos",
    "fade", "fad", "org", "t", "clip", "c", "1c", "2c", "3c", "4c", "1a", "2a", "3a", "4a", "r",
    "be", "b", "i", "kt", "kf", "K", "ko", "k", "shad", "s", "u", "pbo", "p", "q", "fe",
];

pub(super) struct Tag<'a> {
    pub name: &'static str,
    // For t, this is the inner override list, not its timing parameters.
    pub arg: &'a str,
}

fn push<'a>(args: &mut [&'a str; 5], count: &mut usize, value: &'a str) {
    let value = value.trim_end_matches([' ', '\t']);
    if !value.is_empty() && *count < args.len() {
        args[*count] = value;
        *count += 1;
    }
}

pub(super) fn parse(mut input: &str) -> Vec<Tag<'_>> {
    let mut tags = Vec::new();
    while let Some(slash) = input.find('\\') {
        input = input[slash + 1..].trim_start_matches([' ', '\t']);
        let end = input.find(['(', '\\']).unwrap_or(input.len());
        let head = &input[..end];
        input = &input[end..];
        if head.is_empty() {
            continue;
        }
        let mut args = [""; 5];
        let mut count = 0;
        if let Some(mut rest) = input.strip_prefix('(') {
            loop {
                rest = rest.trim_start_matches([' ', '\t']);
                let mut end = rest.find([',', '\\', ')']).unwrap_or(rest.len());
                if rest.as_bytes().get(end) == Some(&b',') {
                    push(&mut args, &mut count, &rest[..end]);
                    rest = &rest[end + 1..];
                    continue;
                }
                if rest.as_bytes().get(end) == Some(&b'\\') {
                    // libass uses the first closing parenthesis, not balanced
                    // nesting. A nested t receives the remaining inner slice.
                    end += rest[end..].find(')').unwrap_or(rest.len() - end);
                }
                push(&mut args, &mut count, &rest[..end]);
                input = if end < rest.len() {
                    &rest[end + 1..]
                } else {
                    ""
                };
                break;
            }
        }
        let Some(&name) = NAMES.iter().find(|&&name| head.starts_with(name)) else {
            // Unknown tags are ignored only after consuming their arguments;
            // backslashes inside those arguments must not become live tags.
            continue;
        };
        let arg = if name == "t" {
            if (1..=4).contains(&count) && args[count - 1].contains('\\') {
                args[count - 1]
            } else {
                ""
            }
        } else {
            if !matches!(
                name,
                "iclip" | "clip" | "move" | "pos" | "fade" | "fad" | "org"
            ) {
                push(&mut args, &mut count, &head[name.len()..]);
            }
            args[0]
        };
        tags.push(Tag { name, arg });
    }
    tags
}

// strtoll-style decimal prefix, then clamped to int32_t by libass mystrtoi32.
pub(super) fn integer_prefix(value: &str) -> i32 {
    let value = value.trim_start_matches(|c: char| c.is_ascii_whitespace());
    let (negative, digits) = if let Some(rest) = value.strip_prefix('-') {
        (true, rest)
    } else {
        (false, value.strip_prefix('+').unwrap_or(value))
    };
    let magnitude = digits
        .bytes()
        .take_while(u8::is_ascii_digit)
        .fold(0_u32, |n, c| {
            n.saturating_mul(10)
                .saturating_add(u32::from(c - b'0'))
                .min(1 << 31)
        });
    let signed = if negative {
        -i64::from(magnitude)
    } else {
        i64::from(magnitude)
    };
    signed.clamp(i64::from(i32::MIN), i64::from(i32::MAX)) as i32
}

#[cfg(test)]
mod tests {
    use super::integer_prefix;

    #[test]
    fn decimal_prefix_conversion_clamps_like_libass() {
        for (input, expected) in [
            ("", 0),
            ("klur4", 0),
            ("  +1.7", 1),
            ("-20junk", -20),
            ("2147483648", i32::MAX),
            ("-999999999999999999999999", i32::MIN),
        ] {
            assert_eq!(integer_prefix(input), expected, "{input}");
        }
    }
}
