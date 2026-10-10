use std::path::{Component, Path, PathBuf};

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

    /// [`normalize`] 返回的是平台原生路径（Windows 上是 `\` 分隔），而这里的断言
    /// 写的是 POSIX 形式，所以先把分隔符统一再比。
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
}
