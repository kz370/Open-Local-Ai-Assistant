@echo off
setlocal EnableExtensions
cd /d "%~dp0"

rem Publish a GitHub release of this repo from the files build-installer.bat
rem put in release\:
rem   release\Open-Local-Assistant-<version>-setup.exe      uploaded as-is
rem   release\Open Local Assistant.exe + the speech DLLs    zipped as the portable version
rem Release notes come from release-notes\<tag>.md, the commit message from
rem commit-message.txt (git-ignored, rewrite it for each release).
rem
rem Usage: upload-release.bat [tag]   (e.g. upload-release.bat v0.1.0)
rem No tag given = read the version from the setup exe name
rem (Open-Local-Assistant-<version>-setup.exe) and use tag v<version>.
set "DIST=release"
set "TAG=%~1"
if not "%TAG%"=="" goto :have_tag

rem Newest setup exe wins if there are several.
set "SETUP="
for /f "delims=" %%F in ('dir /b /a-d /o-d "%DIST%\Open-Local-Assistant-*-setup.exe" 2^>nul') do if not defined SETUP set "SETUP=%%F"
if not defined SETUP (
  echo No %DIST%\Open-Local-Assistant-*-setup.exe found and no tag given.
  echo Run build-installer.bat first.
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

rem Commit and push first, so a new release tag points at the commit these
rem builds came from.
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
