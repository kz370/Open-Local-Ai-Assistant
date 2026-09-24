@echo off
setlocal EnableExtensions
rem Runs against the release repo (its own git repo, so gh picks up its
rem GitHub remote): setup exe in its root, portable exe + DLLs in portable\.
rem Set PUBLISH beforehand to use another folder.
if not defined PUBLISH set "PUBLISH=I:\Development\repos\Open-Local-Ai-Assitant"
cd /d "%PUBLISH%" || (echo Release repo "%PUBLISH%" not found. & exit /b 1)

rem Usage: upload-release.bat [tag]   (e.g. upload-release.bat v0.1.0)
rem No tag given = read the version from the setup exe name
rem (Open-Local-Assistant-<version>-setup.exe) and use tag v<version>.
set "TAG=%~1"
if not "%TAG%"=="" goto :have_tag

rem Newest setup exe wins if there are several.
set "SETUP="
for /f "delims=" %%F in ('dir /b /a-d /o-d "Open-Local-Assistant-*-setup.exe" 2^>nul') do if not defined SETUP set "SETUP=%%F"
if not defined SETUP (
  echo No Open-Local-Assistant-*-setup.exe found and no tag given.
  exit /b 1
)
set "VERSION=%SETUP:Open-Local-Assistant-=%"
set "VERSION=%VERSION:-setup.exe=%"
set "TAG=v%VERSION%"
goto :tag_ready

:have_tag
set "VERSION=%TAG%"
if /i "%VERSION:~0,1%"=="v" set "VERSION=%VERSION:~1%"
set "SETUP=Open-Local-Assistant-%VERSION%-setup.exe"

:tag_ready
echo Using tag %TAG% (version %VERSION%)

set "ZIP=Open-Local-Assistant-%VERSION%-portable-win-x64.zip"
set "ZIPPATH=%TEMP%\%ZIP%"

where gh >nul 2>&1 || (echo GitHub CLI "gh" not found. & exit /b 1)
if not exist "%SETUP%" (echo Missing %SETUP% & exit /b 1)
if not exist "portable\Open Local Assistant.exe" (echo Missing portable\Open Local Assistant.exe & exit /b 1)

rem Commit and push everything in the release repo first, so the release tag
rem points at a commit that has these builds. The commit message is read from
rem commit-message.txt (git-ignored, rewrite it for each release).
set "MSGFILE=commit-message.txt"
where git >nul 2>&1 || (echo git not found. & exit /b 1)
set "DIRTY="
for /f "delims=" %%L in ('git status --porcelain') do set "DIRTY=1"
if defined DIRTY (
  if not exist "%MSGFILE%" (echo Missing %MSGFILE% - write the commit message there first. & exit /b 1)
  echo Committing with message from %MSGFILE%...
  git add -A || (echo git add failed. & exit /b 1)
  git commit -F "%MSGFILE%" || (echo Commit failed. & exit /b 1)
) else (
  echo Nothing new to commit.
)
echo Pushing...
git push origin HEAD || (echo Push failed. & exit /b 1)

echo Zipping portable folder...
if exist "%ZIPPATH%" del /f /q "%ZIPPATH%"
powershell -NoProfile -Command "Compress-Archive -Path 'portable\*' -DestinationPath $env:ZIPPATH -Force" || (echo Zip failed. & exit /b 1)

rem Release notes: release-notes\<tag>.md if present, else GitHub's generated notes
rem (new releases only). An existing release keeps its notes unless the file exists.
set "NOTES=release-notes\%TAG%.md"

gh release view "%TAG%" >nul 2>&1
if errorlevel 1 (
  echo Release %TAG% not found - creating it...
  if exist "%NOTES%" (
    echo Using notes from %NOTES%
    gh release create "%TAG%" --title "%TAG%" --notes-file "%NOTES%" || (echo Create failed. & exit /b 1)
  ) else (
    echo No %NOTES% - using GitHub generated notes.
    gh release create "%TAG%" --title "%TAG%" --generate-notes || (echo Create failed. & exit /b 1)
  )
) else (
  echo Release %TAG% exists - replacing assets.
  if exist "%NOTES%" (
    echo Updating notes from %NOTES%
    gh release edit "%TAG%" --notes-file "%NOTES%" || (echo Notes update failed. & exit /b 1)
  )
)

gh release upload "%TAG%" "%SETUP%" "%ZIPPATH%" --clobber || (echo Upload failed. & exit /b 1)

del /f /q "%ZIPPATH%" >nul 2>&1
echo Done. Uploaded %SETUP% and %ZIP% to %TAG%.
endlocal
