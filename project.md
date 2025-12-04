Project Description: Multi-Branch Commit Extractor
Overview

Multi-Branch Commit Extractor is an interactive Git enhancement tool designed to make it easy to split a large branch into multiple smaller, focused branches in a single operation. The tool provides a git rebase -i-style interface where the user can both define target branches and assign individual commits to those branches using simple shorthand notation.

Rather than running multiple cherry-picks or performing complex rebases, the user edits a single interactive file that declares where each commit should go. The extractor then creates the target branches (if needed) and applies the relevant commits to each one.

Problem Statement

Developers working with large exploratory or long-lived branches often need to break that branch apart into several new branches. Today this requires:

multiple sequences of git cherry-pick,

or multiple interactive rebases,

or temporary staging branches.

Git has no unified, minimal interface for routing commits into multiple branches at once. This leads to repetitive commands and unnecessary conflict resolution.

Proposed Solution

Multi-Branch Commit Extractor introduces a Git subcommand such as:

git extract <target-branch> <target-branch> ...


When invoked, the tool opens an editor containing:

A header section defining the target branches with numeric aliases.

A commit list showing each commit on the current branch, each prefixed by either a branch name or a numeric shorthand.

Example Initial Editor View
target 1 feature1
target 2 feature2

current_branch  abc123 Commit 1
current_branch  def456 Commit 2
current_branch  ghi789 Commit 3
current_branch  jkl012 Commit 4


The user can modify which branch each commit should be routed to by replacing the prefix.

Example After Editing
target 1 feature1
target 2 feature2

1  abc123 Commit 1
current_branch  def456 Commit 2
2  ghi789 Commit 3
1  jkl012 Commit 4


Here:

Commits abc123 and jkl012 go to feature1 (alias 1).

Commit ghi789 goes to feature2 (alias 2).

def456 stays on the current branch.

The editor validates correctness on save and informs the tool how to distribute commits.

Key Behavior

After the interactive file is saved:

Branch Preparation

The tool ensures all target branches exist.

Missing branches are created from a configurable base (default: current branch’s base or mainline).

Commit Routing

Commits assigned to each target branch are applied in order (via cherry-pick or equivalent graph-safe application).

Commits that remain assigned to current_branch stay where they are (or optionally omitted if configured).

Conflict Handling

Conflicts are isolated per target branch.

The tool pauses to allow resolution, then continues.

Summary Output

Reports which commits were added to each branch.

Reports any rewritten commits or branches created.

CLI Usage
Predefining target branches

The CLI allows specifying the target branches directly:

git extract feature1 feature2


This pre-populates the target N <branch> section. The user can still modify these in the editor if needed (e.g., rename or add more).

Basic invocation
git extract


This opens the editor with no predefined targets; the user manually creates the target lines.

Optional flags

Examples:

--base <branch>       # where new branches are created from
--default-current     # whether unassigned commits remain on current branch
--no-current          # drop unassigned commits entirely

Key Features

Interactive “routing file” combining target definition and commit assignment.

Numeric shorthand for fast, ergonomic commit labeling.

Automatic branch creation when needed.

Order-preserving commit application.

Works without modifying the original branch unless explicitly chosen.

Extensible design for future options (reordering, squashing, filtering).

Example Workflow

Run:

git extract feature-login feature-cleanup


Editor opens with:

target 1 feature-login
target 2 feature-cleanup

current_branch  a1b2c3 Add logging
current_branch  d4e5f6 Fix typo
current_branch  112233 Add cleanup script
current_branch  445566 Remove debug prints


User edits:

target 1 feature-login
target 2 feature-cleanup

1 a1b2c3 Add logging
current_branch d4e5f6 Fix typo
2 112233 Add cleanup script
2 445566 Remove debug prints


Save file.

Tool produces two new branches containing the selected commits.

Target Users

Developers decomposing large prototype branches.

Teams practicing topic-branch or PR-based workflows.

Release engineers extracting patches for backports.

Anyone frustrated with repetitive cherry-picks.

Expected Impact

This tool turns a multi-step, error-prone process into a single, predictable operation. The interactive file provides a clear, auditable specification of where each commit should go, significantly improving workflow speed and reliability.

If you want, I can produce next steps such as:

A formal syntax specification,

A sample implementation strategy (in Python, Rust, or Go),

A mockup of the editor file with validation rules,

Or a README format version for a GitHub repository.
