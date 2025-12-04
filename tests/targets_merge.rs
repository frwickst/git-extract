use git_extract::cli::Args;
use git_extract::routing::TargetDefs;

#[test]
fn merges_positional_and_flag_targets_dedup() {
    let args = Args {
        base: None,
        default_current: false,
        no_current: false,
        targets: vec!["a".into(), "b".into()],
        positional_targets: vec!["b".into(), "c".into()],
        editor: None,
        dry_run: false,
        allow_dirty: false,
        routing_file: None,
        r#continue: false,
        abort: false,
        no_chdir_conflict: false,
    };

    let defs = TargetDefs::from_args(&args);
    let branches: Vec<String> = defs.targets.iter().map(|t| t.branch.clone()).collect();
    assert_eq!(branches, vec!["a", "b", "c"]);
    let aliases: Vec<u32> = defs.targets.iter().map(|t| t.alias).collect();
    assert_eq!(aliases, vec![1, 2, 3]);
}
