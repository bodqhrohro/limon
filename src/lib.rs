extern crate itertools;

use itertools::free::join;

pub mod commands;

const ICON_WIDTH: usize = 1;

enum OutputMode {
    PLAIN,
    PANGO,
}

pub struct LimonItem {
    icon: char,
    pre_spaces: usize,
    post_spaces: usize,
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
        format!(
            "{:<width$}{}\n",
            format!("{:>width$}", item.icon, width = item.pre_spaces + ICON_WIDTH),
            item.value,
            width = item.post_spaces + ICON_WIDTH + item.pre_spaces,
        )
    }), &"");

    if let OutputMode::PANGO = mode {
        text.insert_str(0, &format!("<txt><span font='FontAwesome {}'>\n", font_size.expect("Font size not provided")));
        text.push_str("</span></txt>");
    }

    print!("{}", text);

    text
}

pub fn exec_command(command: &commands::Command, arguments: &[&str]) -> LimonItem {
    let (icon, result) = match command {
        commands::Command::Static(command) => (command.icon, (command.call)(arguments)),
        commands::Command::Dynamic(command) => {
            let result = (command.call)(arguments);
            match result {
                Some(result) => (result.icon, Some(result.text)),
                None => (' ', None),
            }
        },
    };

    LimonItem {
        icon: icon,
        value: match result {
            Some(v) => v,
            None => "#ERROR#".to_string(),
        },
        pre_spaces: 0,
        post_spaces: 6,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn _two_test_lines() -> Vec<LimonItem> {
        vec!(
            LimonItem { icon: 'a', value: "tist".to_string(), pre_spaces: 0, post_spaces: 8 },
            LimonItem { icon: 'b', value: "zizd".to_string(), pre_spaces: 1, post_spaces: 4 },
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
        let mut lines = text.lines();

        let langular = Some('<');
        let rangular = Some('>');

        let mut line = lines.next().expect("No first line").chars();
        assert_ne!(line.next(), langular);
        assert_ne!(line.last(), rangular);

        let mut line = lines.last().expect("No last line").chars();
        assert_ne!(line.next(), langular);
        assert_ne!(line.last(), rangular);
    }

    #[test]
    fn output_verify_spaces() {
        let text = output_plain(_two_test_lines());
        let mut lines = text.lines();

        let space = Some(' ');

        let mut line1 = lines.next().expect("No line 1").chars();
        assert_ne!(line1.next(), space);
        assert_eq!(line1.next(), space);
        assert_eq!(line1.nth(6), space);
        assert_ne!(line1.next(), space);
        assert_ne!(line1.last(), space);

        let mut line2 = lines.next().expect("No line 2").chars();
        assert_eq!(line2.next(), space);
        assert_ne!(line2.next(), space);
        assert_eq!(line2.next(), space);
        assert_eq!(line2.nth(2), space);
        assert_ne!(line2.next(), space);
        assert_ne!(line2.last(), space);
    }
}
