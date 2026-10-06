# Publish a release without rebuilding

If a successful workflow run built the Windows files but skipped the release
step, you can publish the files from that run. You do not need to run the build
again.

You need the GitHub CLI (`gh`) installed and signed in, and you need access to
the repository from PowerShell.

## 1. Find the successful build

List recent runs:

```powershell
gh run list --workflow build.yml --limit 10
```

Copy the run ID for the successful run that has the build artifact. Set the
version and run ID below to match that run:

```powershell
$version = "1.2.1"
$tag = "v$version"
$runId = "37476394159"
$dist = "release\v$version"
```

Check that the run succeeded and was built from the same commit as the release
tag. Both commands should show the same commit SHA:

```powershell
gh run view $runId --json status,conclusion,headSha
git fetch origin "refs/tags/${tag}:refs/tags/${tag}"
git rev-parse "${tag}^{commit}"
```

If the run did not succeed, or the commit SHAs differ, do not publish those
files as this version.

## 2. Download that run's files

```powershell
New-Item -ItemType Directory -Force $dist | Out-Null
gh run download $runId --name "open-local-assistant-v$version" --dir $dist
```

The downloaded artifact includes the installer, portable ZIP, and SHA-256
checksum file.

## 3. Create or update the GitHub Release

If the release does **not** exist yet, create it and attach the files:

```powershell
gh release create $tag `
  "$dist\Open-Local-Assistant-$version-setup.exe" `
  "$dist\Open-Local-Assistant-$version-portable-win-x64.zip" `
  "$dist\Open-Local-Assistant-$version-SHA256SUMS.txt" `
  --title $tag `
  --notes-file "release-notes\$tag.md"
```

If the release **already exists**, replace its assets instead:

```powershell
gh release upload $tag `
  "$dist\Open-Local-Assistant-$version-setup.exe" `
  "$dist\Open-Local-Assistant-$version-portable-win-x64.zip" `
  "$dist\Open-Local-Assistant-$version-SHA256SUMS.txt" `
  --clobber
```

To also update the existing release notes:

```powershell
gh release edit $tag --notes-file "release-notes\$tag.md"
```

Check that the release has the files:

```powershell
gh release view $tag
```

This procedure only uploads the already-built files. Do not dispatch the
workflow with `release_tag` for this case; that option starts a new build.
