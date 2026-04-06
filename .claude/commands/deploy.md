Ship current changes through the full deploy pipeline: feature branch → PR → merge triggers release.

## Instructions

Follow these steps in order:

### 1. Check state
- Run `git status` and `git diff` to see what's changed
- If there are no changes, tell the user "Nothing to deploy" and stop
- Run `git log --oneline -5` to see recent commits for message style

### 2. Create feature branch
- Generate a branch name from the changes: `feat/<short-description>` or `fix/<short-description>`
- If already on a feature branch (not `main`), skip branch creation and use the current branch
- Create and switch to the branch: `git checkout -b <branch-name>`

### 3. Commit changes
- Stage all relevant files (avoid .env, credentials, large binaries)
- Write a concise commit message that describes what changed and why
- Do NOT add Co-Authored-By lines
- Commit the changes

### 4. Push and create PR
- Push the branch: `git push origin <branch-name> -u`
- Create a PR to `main` using `gh pr create`:
  - Short title (under 70 chars)
  - Body with `## Summary` (2-3 bullets) and `## Test plan`
  - Do NOT add emoji to the PR

### 5. Merge the PR
- Ask the user: "PR created at <url>. Ready to merge and trigger a release?"
- If user confirms, merge with: `gh pr merge --squash --delete-branch`
- If user declines, stop here — they can merge later

### 6. Confirm release
- After merge, tell the user: "Merged to main. The auto-release workflow will detect the merge, bump the version, tag it, and build binaries for all platforms."
- Show the Actions URL so they can watch: `https://github.com/Eldridge-Morgan/cooper/actions`

## Rules
- Never force push
- Never push directly to main
- Never skip pre-commit hooks
- Never add Co-Authored-By lines to commits
- If anything fails, stop and explain — don't retry blindly
