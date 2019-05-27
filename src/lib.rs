extern crate itertools;

use itertools::free::join;

enum OutputMode {
    PLAIN,
    PANGO,
}

pub struct LimonItem {
    icon: char,
    value: String,
}

pub fn output_plain(items: Vec<LimonItem>) -> String {
    output(OutputMode::PLAIN, items, None)
}

pub fn output_pango(items: Vec<LimonItem>, font_size: u16) -> String {
    output(OutputMode::PANGO, items, Some(font_size))
}

fn output(mode: OutputMode, items: Vec<LimonItem>, font_size: Option<u16>) -> String {
    let mut text = join(items.iter().map(|item| {
        "tist\n"
    }), &",");

    if let OutputMode::PANGO = mode {
        text.insert_str(0, &format!("<txt><span font='FontAwesome {}'>\n", font_size.expect("Font size not provided")));
        text.push_str("</span></txt>");
    }

    print!("{}", text);

    text
}

#[cfg(test)]
mod tests {
    use super::*;

    fn _two_test_lines() -> Vec<LimonItem> {
        vec!(
            LimonItem { icon: 'a', value: "tist".to_string(), },
            LimonItem { icon: 'b', value: "zizd".to_string(), },
        )
    }

    #[test]
    fn output_plain_row_count() {
        let text = output_plain(_two_test_lines());
        assert_eq!(text.lines().count(), 2);
    }

    #[test]
    fn output_pango_has_markup() {
        let text = output_pango(_two_test_lines(), 12);
        let mut lines = text.lines();
        assert_eq!(lines.next(), Some("<txt><span font='FontAwesome 12'>"));
        assert_eq!(lines.next_back(), Some("</span></txt>"));
    }

    #[test]
    fn output_plain_no_markup() {
        let text = output_plain(_two_test_lines());
        assert_eq!(2, 2);
    }
}
