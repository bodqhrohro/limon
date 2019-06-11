pub struct Command
{
    pub icon: char,
    pub call: fn(&[&str]) -> Option<String>,
}

pub const LOADAVG:Command = Command {
    icon: '',
    call: |_| {
        Some("tist".to_string())
    },
};
