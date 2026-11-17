; Inno Setup script for 3D Viewer. Open this file in Inno Setup and press F9 (Build > Compile).
; Build the app first:  cargo build --release --features cad
#define MyAppName "3D Viewer"
#ifndef MyAppVersion
  #define MyAppVersion "1.0.0"
#endif
#define MyAppExe "viewer3d.exe"

[Setup]
AppId={{EE2EB896-D680-4F4F-90FE-7EE14C18DD98}
AppName={#MyAppName}
AppVersion={#MyAppVersion}
AppPublisher=Azizul Haque Shaon
DefaultDirName={autopf}\3D Viewer
DefaultGroupName=3D Viewer
SourceDir=..\..
OutputDir=installer_output
OutputBaseFilename=3DViewer_Setup_{#MyAppVersion}
SetupIconFile=assets\icon.ico
UninstallDisplayIcon={app}\{#MyAppExe}
Compression=lzma2
SolidCompression=yes
WizardStyle=modern
ArchitecturesAllowed=x64compatible
ArchitecturesInstallIn64BitMode=x64compatible
PrivilegesRequiredOverridesAllowed=dialog
ChangesAssociations=yes

[Tasks]
Name: "desktopicon"; Description: "Create a desktop shortcut"
Name: "assoc"; Description: "Add 3D Viewer to the ""Open with"" list for 3D files"

[Files]
Source: "target\release\viewer3d.exe"; DestDir: "{app}"; Flags: ignoreversion

[Icons]
Name: "{group}\3D Viewer"; Filename: "{app}\{#MyAppExe}"
Name: "{autodesktop}\3D Viewer"; Filename: "{app}\{#MyAppExe}"; Tasks: desktopicon

[Registry]
Root: HKA; Subkey: "Software\Classes\3DViewer.File"; ValueType: string; ValueData: "3D Model"; Flags: uninsdeletekey; Tasks: assoc
Root: HKA; Subkey: "Software\Classes\3DViewer.File\DefaultIcon"; ValueType: string; ValueData: "{app}\{#MyAppExe},0"; Tasks: assoc
Root: HKA; Subkey: "Software\Classes\3DViewer.File\shell\open\command"; ValueType: string; ValueData: """{app}\{#MyAppExe}"" ""%1"""; Tasks: assoc
Root: HKA; Subkey: "Software\Classes\.stl\OpenWithProgids"; ValueType: string; ValueName: "3DViewer.File"; ValueData: ""; Flags: uninsdeletevalue; Tasks: assoc
Root: HKA; Subkey: "Software\Classes\.obj\OpenWithProgids"; ValueType: string; ValueName: "3DViewer.File"; ValueData: ""; Flags: uninsdeletevalue; Tasks: assoc
Root: HKA; Subkey: "Software\Classes\.ply\OpenWithProgids"; ValueType: string; ValueName: "3DViewer.File"; ValueData: ""; Flags: uninsdeletevalue; Tasks: assoc
Root: HKA; Subkey: "Software\Classes\.off\OpenWithProgids"; ValueType: string; ValueName: "3DViewer.File"; ValueData: ""; Flags: uninsdeletevalue; Tasks: assoc
Root: HKA; Subkey: "Software\Classes\.gltf\OpenWithProgids"; ValueType: string; ValueName: "3DViewer.File"; ValueData: ""; Flags: uninsdeletevalue; Tasks: assoc
Root: HKA; Subkey: "Software\Classes\.glb\OpenWithProgids"; ValueType: string; ValueName: "3DViewer.File"; ValueData: ""; Flags: uninsdeletevalue; Tasks: assoc
Root: HKA; Subkey: "Software\Classes\.3mf\OpenWithProgids"; ValueType: string; ValueName: "3DViewer.File"; ValueData: ""; Flags: uninsdeletevalue; Tasks: assoc
Root: HKA; Subkey: "Software\Classes\.amf\OpenWithProgids"; ValueType: string; ValueName: "3DViewer.File"; ValueData: ""; Flags: uninsdeletevalue; Tasks: assoc
Root: HKA; Subkey: "Software\Classes\.dae\OpenWithProgids"; ValueType: string; ValueName: "3DViewer.File"; ValueData: ""; Flags: uninsdeletevalue; Tasks: assoc
Root: HKA; Subkey: "Software\Classes\.wrl\OpenWithProgids"; ValueType: string; ValueName: "3DViewer.File"; ValueData: ""; Flags: uninsdeletevalue; Tasks: assoc
Root: HKA; Subkey: "Software\Classes\.3ds\OpenWithProgids"; ValueType: string; ValueName: "3DViewer.File"; ValueData: ""; Flags: uninsdeletevalue; Tasks: assoc
Root: HKA; Subkey: "Software\Classes\.bim\OpenWithProgids"; ValueType: string; ValueName: "3DViewer.File"; ValueData: ""; Flags: uninsdeletevalue; Tasks: assoc
Root: HKA; Subkey: "Software\Classes\.fbx\OpenWithProgids"; ValueType: string; ValueName: "3DViewer.File"; ValueData: ""; Flags: uninsdeletevalue; Tasks: assoc
Root: HKA; Subkey: "Software\Classes\.step\OpenWithProgids"; ValueType: string; ValueName: "3DViewer.File"; ValueData: ""; Flags: uninsdeletevalue; Tasks: assoc
Root: HKA; Subkey: "Software\Classes\.stp\OpenWithProgids"; ValueType: string; ValueName: "3DViewer.File"; ValueData: ""; Flags: uninsdeletevalue; Tasks: assoc
Root: HKA; Subkey: "Software\Classes\.iges\OpenWithProgids"; ValueType: string; ValueName: "3DViewer.File"; ValueData: ""; Flags: uninsdeletevalue; Tasks: assoc
Root: HKA; Subkey: "Software\Classes\.igs\OpenWithProgids"; ValueType: string; ValueName: "3DViewer.File"; ValueData: ""; Flags: uninsdeletevalue; Tasks: assoc
Root: HKA; Subkey: "Software\Classes\.brep\OpenWithProgids"; ValueType: string; ValueName: "3DViewer.File"; ValueData: ""; Flags: uninsdeletevalue; Tasks: assoc
Root: HKA; Subkey: "Software\Classes\.brp\OpenWithProgids"; ValueType: string; ValueName: "3DViewer.File"; ValueData: ""; Flags: uninsdeletevalue; Tasks: assoc
Root: HKA; Subkey: "Software\Classes\.fcstd\OpenWithProgids"; ValueType: string; ValueName: "3DViewer.File"; ValueData: ""; Flags: uninsdeletevalue; Tasks: assoc

[Run]
Filename: "{app}\{#MyAppExe}"; Description: "Launch 3D Viewer"; Flags: nowait postinstall skipifsilent
