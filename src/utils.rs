pub fn trim_trailing_newline(s: &mut String) -> () {
    if s.ends_with('\n') {
        s.pop();
    }
}
