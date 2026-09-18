; ---------------------------------------------------------------------------
; Open Local Assistant - Inno Setup 6 script (all users, C:\Program Files)
;
; Built by build-installer.bat, which passes:
;   /DAppVersion=...  /DSourceExe=...  /DOutputDir=...
; ---------------------------------------------------------------------------

#define AppName "Open Local Assistant"

[Setup]
AppId={{E4B7C2A1-5F3D-4E8A-9C1B-2D3E4F5A6B7C}}
AppName={#AppName}
AppVersion={#AppVersion}
AppPublisher=Local Assistant
DefaultDirName={autopf}\Open Local Assistant
PrivilegesRequired=admin
OutputDir={#OutputDir}
OutputBaseFilename=Open-Local-Assistant-{#AppVersion}-setup
SetupIconFile=..\src-tauri\icons\icon.ico
UninstallDisplayIcon={app}\Open Local Assistant.exe
Compression=lzma2/max
SolidCompression=yes
ArchitecturesAllowed=x64compatible
ArchitecturesInstallIn64BitMode=x64compatible
WizardStyle=modern

[Files]
Source: "{#SourceExe}"; DestDir: "{app}"; DestName: "Open Local Assistant.exe"; Flags: ignoreversion
;   The speech models run on sherpa-onnx / ONNX Runtime, which are linked
;   dynamically so an optional CUDA pack can replace them at launch.
Source: "{#LibDir}\sherpa-onnx-c-api.dll"; DestDir: "{app}"; Flags: ignoreversion
Source: "{#LibDir}\sherpa-onnx-cxx-api.dll"; DestDir: "{app}"; Flags: ignoreversion
Source: "{#LibDir}\onnxruntime.dll"; DestDir: "{app}"; Flags: ignoreversion
Source: "{#LibDir}\onnxruntime_providers_shared.dll"; DestDir: "{app}"; Flags: ignoreversion
Source: "..\src-tauri\icons\icon.ico"; DestDir: "{app}"; DestName: "icon.ico"; Flags: ignoreversion

[Icons]
Name: "{autoprograms}\Open Local Assistant"; Filename: "{app}\Open Local Assistant.exe"
Name: "{autodesktop}\Open Local Assistant"; Filename: "{app}\Open Local Assistant.exe"; Tasks: desktopicon

[Tasks]
Name: desktopicon; Description: "Create a &desktop icon"; Flags: unchecked

[Run]
Filename: "{app}\Open Local Assistant.exe"; Description: "Launch Open Local Assistant"; Flags: nowait postinstall skipifsilent
