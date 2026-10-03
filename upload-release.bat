@echo off
setlocal EnableExtensions
cd /d "%~dp0"

rem Publish a GitHub release of this repo from the files build-installer.bat
rem put in release\v<version>\:
rem   release\v<version>\Open-Local-Assistant-<version>-setup.exe   uploaded as-is
rem   release\v<version>\Open Local Assistant.exe + speech DLLs    zipped as the
rem                                                              portable version
rem Release notes come from release-notes\<tag>.md, the commit message from
rem commit-message.txt (git-ignored, rewrite it for each release).
rem
rem Usage: upload-release.bat [tag]   (e.g. upload-release.bat v1.0.0)
rem No tag given = read the version from the newest setup exe under
rem release\ (release\v<version>\Open-Local-Assistant-<version>-setup.exe)
rem and use tag v<version>.
set "RELEASEDIR=release"
set "TAG=%~1"
if not "%TAG%"=="" goto :have_tag

rem Newest setup exe wins if there are several. Searched recursively because each
rem release keeps its own release\v<version>\ folder.
set "SETUP="
set "VERSION="
set "DIST="
for /f "delims=" %%F in ('powershell -NoProfile -Command "$f = Get-ChildItem -Path '%RELEASEDIR%' -Recurse -File -Filter 'Open-Local-Assistant-*-setup.exe' -ErrorAction SilentlyContinue | Sort-Object LastWriteTime -Descending | Select-Object -First 1; if ($f) { $f.FullName }"') do if not defined SETUP set "SETUP=%%F"
if not defined SETUP (
  echo No Open-Local-Assistant-*-setup.exe found under %RELEASEDIR%\ and no tag given.
  echo Run build-installer.bat first.
  exit /b 1
)
for %%P in ("%SETUP%") do (
  set "DIST=%%~dpP"
  set "SETUP=%%~nxP"
)
if "%DIST:~-1%"=="\" set "DIST=%DIST:~0,-1%"
set "VERSION=%SETUP:Open-Local-Assistant-=%"
set "VERSION=%VERSION:-setup.exe=%"
set "TAG=v%VERSION%"
goto :tag_ready

:have_tag
set "VERSION=%TAG%"
if /i "%VERSION:~0,1%"=="v" set "VERSION=%VERSION:~1%"
set "SETUP=Open-Local-Assistant-%VERSION%-setup.exe"
set "DIST=%RELEASEDIR%\v%VERSION%"
if exist "%DIST%\%SETUP%" goto :tag_ready
rem No per-release folder (built before this layout): fall back to release\.
if exist "%RELEASEDIR%\%SETUP%" set "DIST=%RELEASEDIR%"

:tag_ready
echo Using tag %TAG% (version %VERSION%)

set "SETUPPATH=%DIST%\%SETUP%"
set "ZIP=Open-Local-Assistant-%VERSION%-portable-win-x64.zip"
set "ZIPPATH=%TEMP%\%ZIP%"
set "STAGE=%TEMP%\open-local-assistant-portable"
set "PORTABLE_FILES="Open Local Assistant.exe" sherpa-onnx-c-api.dll sherpa-onnx-cxx-api.dll onnxruntime.dll onnxruntime_providers_shared.dll"

where gh >nul 2>&1 || (echo GitHub CLI "gh" not found. & exit /b 1)
where git >nul 2>&1 || (echo git not found. & exit /b 1)
if not exist "%SETUPPATH%" (echo Missing %SETUPPATH% - run build-installer.bat first. & exit /b 1)
for %%f in (%PORTABLE_FILES%) do (
  if not exist "%DIST%\%%~f" (echo Missing %DIST%\%%~f - run build-installer.bat first. & exit /b 1)
)

rem Commit and push first, then move the release tag to this commit so GitHub's
rem source-code archives contain the same source as the uploaded build.
set "MSGFILE=commit-message.txt"
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
echo Updating tag %TAG% to the published commit...
git push origin --force "HEAD:refs/tags/%TAG%" || (echo Tag update failed. & exit /b 1)

rem Only the portable files go in the zip, not the setup exe next to them.
echo Zipping portable version...
if exist "%STAGE%" rmdir /s /q "%STAGE%"
mkdir "%STAGE%" || (echo Could not create %STAGE%. & exit /b 1)
for %%f in (%PORTABLE_FILES%) do copy /y "%DIST%\%%~f" "%STAGE%\" >nul || (echo Could not copy %%~f. & exit /b 1)
if exist "%ZIPPATH%" del /f /q "%ZIPPATH%"
powershell -NoProfile -Command "Compress-Archive -Path (Join-Path $env:STAGE '*') -DestinationPath $env:ZIPPATH -Force" || (echo Zip failed. & exit /b 1)
rmdir /s /q "%STAGE%"

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

gh release upload "%TAG%" "%SETUPPATH%" "%ZIPPATH%" --clobber || (echo Upload failed. & exit /b 1)

del /f /q "%ZIPPATH%" >nul 2>&1
echo Done. Uploaded %SETUP% and %ZIP% to %TAG%.
endlocal & exit /b 0
