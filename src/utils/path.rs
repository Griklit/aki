use std::path::{Component, Path, PathBuf};

/// 把一段名字变成安全的文件名（去掉路径分隔符等非法字符）。
#[must_use]
pub fn sanitize_name(name: &str) -> String {
    sanitize_filename::sanitize(name)
}

/// 把一段名字变成安全的相对路径段（不会跨越子目录）。
#[must_use]
pub fn sanitized_segment(name: &str) -> PathBuf {
    PathBuf::from(sanitize_name(name))
}

pub fn normalize(path: impl AsRef<Path>) -> PathBuf {
    let path = path.as_ref();
    let mut stack: Vec<Component> = Vec::new();
    for comp in path.components() {
        match comp {
            Component::CurDir => {}
            Component::ParentDir => {
                match stack.last() {
                    Some(Component::Normal(_)) => {
                        stack.pop();
                    }
                    Some(Component::Prefix(_) | Component::RootDir) => {
                        // root 不能 pop
                    }
                    _ => {
                        // ⭐关键修复：保留 ..
                        stack.push(Component::ParentDir);
                    }
                }
            }
            Component::RootDir | Component::Prefix(_) | Component::Normal(_) => {
                stack.push(comp);
            }
        }
    }
    let mut result = PathBuf::with_capacity(stack.len());
    for c in stack {
        result.push(c.as_os_str());
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 把路径统一成 POSIX 形式再比较。
    fn norm(input: &str) -> String {
        normalize(input)
            .to_string_lossy()
            .replace(std::path::MAIN_SEPARATOR, "/")
    }

    #[test]
    fn test_dot_segments() {
        assert_eq!(norm("./a/./b"), "a/b");
        assert_eq!(norm("a/./b/."), "a/b");
    }

    #[test]
    fn test_double_slash() {
        assert_eq!(norm("a//b///c"), "a/b/c");
    }

    #[test]
    fn test_parent_dir_simple() {
        assert_eq!(norm("a/b/../c"), "a/c");
    }

    #[test]
    fn test_parent_dir_multiple() {
        assert_eq!(norm("a/b/c/../../d"), "a/d");
    }

    #[test]
    fn test_parent_dir_exceeding() {
        assert_eq!(norm("a/../../b"), "../b");
        assert_eq!(norm("../../a"), "../../a");
    }

    #[test]
    fn test_absolute_path() {
        assert_eq!(norm("/a/b/../c"), "/a/c");
    }

    #[test]
    fn test_root_only_parent() {
        assert_eq!(norm("/../a"), "/a");
    }

    #[test]
    fn test_empty_and_dot() {
        assert_eq!(norm("."), "");
        assert_eq!(norm("./"), "");
    }

    #[test]
    fn test_complex_case() {
        assert_eq!(norm("/a/./b/../../c/../d"), "/d");
    }

    #[test]
    fn test_windows_prefix_basic() {
        let p = normalize("C:\\a\\..\\b");
        let s = p.to_string_lossy();
        assert!(s.contains("C:"));
        assert!(s.ends_with('b'));
    }

    #[test]
    fn sanitized_segment_never_escapes_the_directory() {
        // 文件名里的分隔符被去掉，因此不会变成子目录或跳到上一级。
        assert_eq!(sanitized_segment("a/b"), PathBuf::from("ab"));
        assert_eq!(sanitized_segment("../x"), PathBuf::from("..x"));
        assert_eq!(
            sanitized_segment("[2010-10-04] 侵略!イカ娘"),
            PathBuf::from("[2010-10-04] 侵略!イカ娘")
        );
    }
}
