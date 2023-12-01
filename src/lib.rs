extern crate itertools;

pub mod commands;
pub mod utils;

use itertools::free::join;

const ICON_WIDTH: usize = 1;

pub struct LimonItem {
    icon: char,
    pub bar: Option<u8>,
    pre_spaces: usize,
    post_spaces: usize,
    value: String,
}

macro_rules! format_icon { ($i:expr, $pre_spaces:expr) => { format!("{:>width$}", $i, width = $pre_spaces + ICON_WIDTH) } }

pub fn output_plain(items: Vec<LimonItem>) -> String {
    let text = join(items.iter().map(|item| format!("{}\t{}\n", format_icon!(item.icon, item.pre_spaces), item.value)), &"");

    print!("{}", text);

    text
}

pub fn output_pango(items: Vec<LimonItem>, icon_font_size: u16, text_font_size: u16, bar: Option<u8>) -> String {
    let mut text = join(items.iter().map(|item| {
        format!(
            "<span font='FontAwesome {}'>{}</span>\t<span font='VCR OSD Mono {}'>{}</span>",
            icon_font_size,
            format_icon!(item.icon, item.pre_spaces),
            text_font_size,
            item.value,
        )
    }), &"\n");

    text.insert_str(0, "<txt>");
    text.push_str("</txt>");
    if let Some(bar) = bar {
        text.push_str(&format!("<bar>{}</bar>", bar));
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
        let text = output_pango(_two_test_lines(), 12, 11, None);
        let mut lines = text.lines();
        assert!(lines.next().unwrap().starts_with("<txt><span font='FontAwesome 12'>"));
        assert!(lines.next_back().unwrap().ends_with("</span></txt>"));
    }

    #[test]
    fn output_pango_with_bar() {
        let text = output_pango(_two_test_lines(), 12, 11, Some(23));
        let mut lines = text.lines();
        assert!(lines.next().unwrap().starts_with("<txt><span font='FontAwesome 12'>"));
        assert!(lines.next_back().unwrap().ends_with("</span></txt><bar>23</bar>"));
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
        let tab = Some('\t');

        let mut line1 = lines.next().expect("No line 1").chars();
        assert_ne!(line1.next(), space);
        assert_eq!(line1.next(), tab);
        assert_ne!(line1.next(), space);
        assert_ne!(line1.last(), space);

        let mut line2 = lines.next().expect("No line 2").chars();
        assert_eq!(line2.next(), space);
        assert_ne!(line2.next(), space);
        assert_eq!(line2.next(), tab);
        assert_ne!(line2.next(), space);
        assert_ne!(line2.last(), space);
    }
}
