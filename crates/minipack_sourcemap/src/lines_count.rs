use memchr::memmem;

#[inline]
pub fn lines_count(str: &str) -> u32 {
  u32::try_from(memmem::find_iter(str.as_bytes(), "\n").count()).unwrap()
}

#[test]
fn test() {
  assert_eq!(lines_count("a\nb\nc"), 2);
  assert_eq!(lines_count("a\nb\nc\n"), 3);
  assert_eq!(lines_count("a"), 0);
}
