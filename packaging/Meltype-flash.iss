; SPDX-License-Identifier: GPL-3.0-or-later
; Copyright (C) 2026 appipinopi
#ifndef AppVersion
  #define AppVersion "0.1.0-pre.2"
#endif
#ifndef SourceDir
  #error SourceDir is required (audited Rust package)
#endif
#ifndef OutputDir
  #define OutputDir "..\dist"
#endif
[Setup]
AppId=Meltype-flash
AppName=Meltype-flash
AppVersion={#AppVersion}
AppPublisher=appipinopi
AppPublisherURL=https://github.com/appipinopi/Meltype
AppSupportURL=https://github.com/appipinopi/Meltype/issues
DefaultDirName={autopf}\Meltype-flash\installer
DisableDirPage=yes
DisableProgramGroupPage=yes
PrivilegesRequired=admin
ArchitecturesAllowed=x64compatible
ArchitecturesInstallIn64BitMode=x64compatible
OutputDir={#OutputDir}
OutputBaseFilename=Meltype-flash-{#AppVersion}-setup
Compression=lzma2
SolidCompression=yes
WizardStyle=modern
LicenseFile={#SourceDir}\LICENSE
UninstallDisplayIcon={app}\meltype-flash.exe
CloseApplications=no
RestartApplications=no
SetupLogging=yes
[Languages]
Name: "japanese"; MessagesFile: "compiler:Languages\Japanese.isl"
Name: "english"; MessagesFile: "compiler:Default.isl"
[Files]
Source: "{#SourceDir}\*"; DestDir: "{app}"; Flags: ignoreversion recursesubdirs createallsubdirs
[Icons]
Name: "{commonprograms}\Meltype-flash 設定"; Filename: "{app}\meltype-flash.exe"
[Code]
function Manage(Action: String): Boolean;
var Code: Integer;
begin
  Result := Exec(ExpandConstant('{sys}\WindowsPowerShell\v1.0\powershell.exe'),
    '-NoProfile -ExecutionPolicy Bypass -File "' + ExpandConstant('{app}\Manage-Windows.ps1') +
    '" -Action ' + Action + ' -Package "' + ExpandConstant('{app}') + '"',
    ExpandConstant('{app}'), SW_HIDE, ewWaitUntilTerminated, Code);
  Result := Result and (Code = 0);
end;
procedure CurStepChanged(CurStep: TSetupStep);
begin
  if CurStep = ssPostInstall then
    if not Manage('Install') then
      RaiseException('Meltype-flash の IME 登録に失敗しました。セットアップログを確認してください。');
end;
function InitializeUninstall(): Boolean;
begin
  Result := Manage('Uninstall');
  if not Result then
    MsgBox('IME の解除に失敗しました。アプリを閉じて再実行してください。設定・学習データは保持します。', mbError, MB_OK);
end;
