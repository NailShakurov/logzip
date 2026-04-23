use std::path::PathBuf;

pub struct Sandbox {
    pub allowed: Vec<PathBuf>,
}

impl Sandbox {
    pub fn new(dirs: Vec<PathBuf>) -> Result<Self, String> {
        let targets = if dirs.is_empty() {
            vec![std::env::current_dir().map_err(|e| e.to_string())?]
        } else {
            dirs
        };

        let mut allowed = Vec::new();
        for dir in targets {
            // canonicalize базовых путей обязателен: на Windows canonicalize возвращает
            // UNC-пути (\\?\C:\...), поэтому сравниваем яблоки с яблоками.
            match std::fs::canonicalize(&dir) {
                Ok(canon) => allowed.push(canon),
                Err(e) => return Err(format!("Invalid --allow-dir {:?}: {}", dir, e)),
            }
        }
        Ok(Self { allowed })
    }

    pub fn validate(&self, path: &str) -> Result<PathBuf, String> {
        let canonical = std::fs::canonicalize(path)
            .map_err(|e| format!("Cannot resolve path '{}': {}", path, e))?;

        if self.allowed.iter().any(|a| canonical.starts_with(a)) {
            Ok(canonical)
        } else {
            Err(format!("Path '{}' is outside allowed directories", path))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::env;

    fn make_temp_file(dir: &PathBuf, name: &str) -> PathBuf {
        let p = dir.join(name);
        fs::write(&p, b"test log content").unwrap();
        p
    }

    #[test]
    fn test_path_inside_allowed_dir() {
        let tmp = env::temp_dir();
        let file = make_temp_file(&tmp, "logzip_test_valid.log");
        let sandbox = Sandbox::new(vec![tmp.clone()]).unwrap();
        assert!(sandbox.validate(file.to_str().unwrap()).is_ok());
        fs::remove_file(file).unwrap();
    }

    #[test]
    fn test_path_outside_allowed_dir() {
        let tmp = env::temp_dir();
        let subdir = tmp.join("logzip_sandbox_allowed");
        fs::create_dir_all(&subdir).unwrap();
        let file = make_temp_file(&tmp, "logzip_test_outside.log");

        let sandbox = Sandbox::new(vec![subdir.clone()]).unwrap();
        assert!(sandbox.validate(file.to_str().unwrap()).is_err());

        fs::remove_file(file).unwrap();
        fs::remove_dir(subdir).unwrap();
    }

    #[test]
    fn test_path_traversal_rejected() {
        let tmp = env::temp_dir();
        let allowed = tmp.join("logzip_sandbox_traversal_test");
        fs::create_dir_all(&allowed).unwrap();
        let sandbox = Sandbox::new(vec![allowed.clone()]).unwrap();

        let traversal = format!("{}/../../etc/passwd", allowed.display());
        assert!(sandbox.validate(&traversal).is_err());

        fs::remove_dir_all(allowed).unwrap();
    }

    #[test]
    fn test_empty_dirs_defaults_to_cwd() {
        let sandbox = Sandbox::new(vec![]).unwrap();
        let expected = std::fs::canonicalize(env::current_dir().unwrap()).unwrap();
        assert_eq!(sandbox.allowed.len(), 1);
        assert_eq!(sandbox.allowed[0], expected);
    }

    #[test]
    fn test_invalid_allow_dir_returns_error() {
        let result = Sandbox::new(vec![PathBuf::from("/nonexistent/path/xyz")]);
        assert!(result.is_err());
    }
}
