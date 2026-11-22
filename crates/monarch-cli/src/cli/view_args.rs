//! Arguments for `monarch view`.

use std::path::PathBuf;

use clap::Args;

/// The file to show.
#[derive(Debug, Clone, PartialEq, Args)]
pub struct ViewArgs {
    /// The file, from the current directory or absolute.
    pub path: PathBuf,
}

#[cfg(test)]
mod tests {
    use clap::Parser;

    use super::*;

    #[derive(Parser)]
    struct Wrap {
        #[command(flatten)]
        args: ViewArgs,
    }

    #[test]
    fn a_path_is_needed() {
        let w = Wrap::try_parse_from(["x", "src/main.rs"]).expect("parse");
        assert_eq!(w.args.path, PathBuf::from("src/main.rs"));
        assert!(Wrap::try_parse_from(["x"]).is_err());
    }
}
