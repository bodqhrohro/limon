extern crate itertools;

pub mod commands;
pub mod utils;

use utils::trim_trailing_newline;

use itertools::free::join;

const ICON_WIDTH: usize = 1;

enum OutputMode {
    PLAIN,
    PANGO,
}

pub struct LimonItem {
    icon: char,
    pub bar: Option<u8>,
    pre_spaces: usize,
    post_spaces: usize,
    value: String,
}

pub fn output_plain(items: Vec<LimonItem>) -> String {
    output(OutputMode::PLAIN, items, None, None)
}

pub fn output_pango(items: Vec<LimonItem>, font_size: u16, bar: Option<u8>) -> String {
    output(OutputMode::PANGO, items, Some(font_size), bar)
}

fn output(mode: OutputMode, items: Vec<LimonItem>, font_size: Option<u16>, bar: Option<u8>) -> String {
    let mut text = join(items.iter().map(|item| {
        format!(
            "{:<width$}{}\n",
            format!("{:>width$}", item.icon, width = item.pre_spaces + ICON_WIDTH),
            item.value,
            width = item.post_spaces + ICON_WIDTH + item.pre_spaces,
        )
    }), &"");

    if let OutputMode::PANGO = mode {
        text.insert_str(0, &format!("<txt><span font='FontAwesome {}'>", font_size.expect("Font size not provided")));
        trim_trailing_newline(&mut text);
        text.push_str("</span></txt>");
        if let Some(bar) = bar {
            text.push_str(&format!("<bar>{}</bar>", bar));
        }
    }

    print!("{}", text);

    text
}

pub fn exec_command(command: &commands::Command, arguments: &[&str]) -> LimonItem {
    let (icon, result, bar, pre_spaces, post_spaces) = match command {
        commands::Command::Static(command) => (command.icon, (command.call)(arguments), None, command.pre_spaces, command.post_spaces),
        commands::Command::Dynamic(command) => {
            let result = (command.call)(arguments);
            match result {
                Some(result) => (result.icon, Some(result.text), result.bar, result.pre_spaces, result.post_spaces),
                None => (' ', None, None, 0, 0),
            }
        },
    };

    LimonItem {
        icon: icon,
        value: match result {
            Some(v) => v,
            None => "#ERROR#".to_string(),
        },
        bar: bar,
        pre_spaces: pre_spaces,
        post_spaces: post_spaces,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn _two_test_lines() -> Vec<LimonItem> {
        vec!(
            LimonItem { icon: 'a', value: "tist".to_string(), bar: None, pre_spaces: 0, post_spaces: 8 },
            LimonItem { icon: 'b', value: "zizd".to_string(), bar: None, pre_spaces: 1, post_spaces: 4 },
        )
    }

    #[test]
    fn output_plain_row_count() {
        let text = output_plain(_two_test_lines());
        assert_eq!(text.lines().count(), 2);
    }

    #[test]
    fn output_pango_has_markup() {
        let text = output_pango(_two_test_lines(), 12, None);
        let mut lines = text.lines();
        assert_eq!(lines.next(), Some("<txt><span font='FontAwesome 12'>"));
        assert_eq!(lines.next_back(), Some("</span></txt>"));
    }

    #[test]
    fn output_pango_with_bar() {
        let text = output_pango(_two_test_lines(), 12, Some(23));
        let mut lines = text.lines();
        assert_eq!(lines.next(), Some("<txt><span font='FontAwesome 12'>"));
        assert_eq!(lines.next_back(), Some("</span></txt><bar>23</bar>"));
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
