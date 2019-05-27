extern crate itertools;

use itertools::free::join;

pub enum OutputMode {
    PLAIN,
    PANGO,
}

pub struct LimonItem {
    icon: char,
    value: String,
}

pub fn output(mode: OutputMode, items: Vec<LimonItem>) -> String {
    let lines = join(items.iter().map(|item| {
        "tist\n"
    }), &",");

    print!("{}", lines);

    lines
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn output_plain_row_count() {
        let text = output(OutputMode::PLAIN, vec!(
            LimonItem { icon: 'a', value: "tist".to_string(), },
            LimonItem { icon: 'b', value: "zizd".to_string(), },
        ));
        assert_eq!(text.lines().count(), 2);
    }
}
