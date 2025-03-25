# Contributing to Ixa

## Commit messages

This project uses [Conventional Commits](https://release-plz.dev/docs/changelog/format#how-should-i-write-my-commits)
in order to auto-generate change log entries and detect version changes.

Keep the following in mind when merging PRs to `main`:

### Squash commits

Generally, you should use the squash button if you only need to land a single commit (if more than one commit is landing, make sure you
format them in git before rebasing/merging). Don't land commits like "review fix" on `main`.

### Make commits readable

Your message is going to show up the next version release changelog, so make it somewhat reasonable.

### Include a conventional commit prefix

- `fix:`: represents bug fixes, and results in a SemVer `patch` bump (e.g., v0.1.0 -> v0.1.1).
- `feat:`: represents a new feature, and results in a SemVer `minor` bump (e.g., v0.1.3 -> v0.2.0)
- `<prefix>!`: (e.g. `feat!:`): represents a breaking change (indicated by the !) and results in a SemVer major bump (e.g., v0.1.0 -> v1.0.0)

### Include fix message to auto-close related issues

In the body section of the commit, add `fix #xxx` where xxx is the number of a commit to auto-close it.

See [GitHub documentation on linking a pull request to an issue](https://docs.github.com/en/issues/tracking-your-work-with-issues/using-issues/linking-a-pull-request-to-an-issue#linking-a-pull-request-to-an-issue-using-a-keyword)

## Releasing Ixa

This project uses release-plz to automate release publishing.

### Create a new release PR

To create a new release, go to the Actions tab of the repo and open the [Create Release PR](https://github.com/CDCgov/ixa/actions/workflows/release-plz-pr.yaml) workflow.

Click the "Run workflow" and choose `main` to generate a new release PR

### Review the release PR

Take a look at the version bump and changelog that is created. You can pull the branch and push
additional manual changes to the changelog as needed before merging.

Approve and merge the PR when everything looks good.

## Check crates.io

Take a look at crates.io to ensure that the new version of ixa got published. If there was an issue,
check the actions tab for any failed workflows.
