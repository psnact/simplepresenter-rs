; Inno Setup script for the Rust build. Expects dist\SimplePresenter\ with SimplePresenter.exe and
; resources\ (the GitHub Actions workflow creates it). Build: iscc /DAppVer=2.0.0 packaging\SimplePresenter.iss
#ifndef AppVer
#define AppVer "2.0.0"
#endif

[Setup]
; Same AppId as the Qt installer, so this upgrades an existing installation in place.
AppId={{8C4B7F6D-2D74-4B1E-9B86-1234567890AB}
AppName=SimplePresenter
AppVersion={#AppVer}
AppPublisher=ZaelyApps
AppPublisherURL=https://github.com/psnact
AppSupportURL=https://github.com/psnact/SimplePresenter/issues
AppUpdatesURL=https://github.com/psnact/SimplePresenter/releases
VersionInfoCompany=ZaelyApps
VersionInfoDescription=SimplePresenter
VersionInfoVersion={#AppVer}
ArchitecturesAllowed=x64compatible
ArchitecturesInstallIn64BitMode=x64compatible
DefaultDirName={autopf}\SimplePresenter
DefaultGroupName=SimplePresenter
OutputDir=..\dist
OutputBaseFilename=SimplePresenter-Setup
Compression=lzma2
SolidCompression=yes
DisableDirPage=auto
CloseApplications=yes
SetupIconFile=Logo.ico
UninstallDisplayIcon={app}\SimplePresenter.exe

[InstallDelete]
; Leftovers from the Qt version.
Type: files; Name: "{app}\Qt6*.dll"
Type: files; Name: "{app}\QtWebEngineProcess.exe"
Type: files; Name: "{app}\D3Dcompiler_47.dll"
Type: files; Name: "{app}\opengl32sw.dll"
Type: files; Name: "{app}\icu*.dll"
Type: files; Name: "{app}\av*.dll"
Type: files; Name: "{app}\sw*.dll"
Type: files; Name: "{app}\deckcraft-cli.exe"
Type: filesandordirs; Name: "{app}\platforms"
Type: filesandordirs; Name: "{app}\imageformats"
Type: filesandordirs; Name: "{app}\iconengines"
Type: filesandordirs; Name: "{app}\styles"
Type: filesandordirs; Name: "{app}\tls"
Type: filesandordirs; Name: "{app}\multimedia"
Type: filesandordirs; Name: "{app}\networkinformation"
Type: filesandordirs; Name: "{app}\position"
Type: filesandordirs; Name: "{app}\qml"
Type: filesandordirs; Name: "{app}\generic"
Type: filesandordirs; Name: "{app}\translations"
Type: filesandordirs; Name: "{app}\resources"

[Files]
Source: "..\dist\SimplePresenter\*"; DestDir: "{app}"; Flags: recursesubdirs createallsubdirs ignoreversion
Source: "Logo.ico"; DestDir: "{app}"; Flags: ignoreversion

[Registry]
Root: HKCR; Subkey: ".spp"; ValueType: string; ValueData: "SimplePresenter.Playlist"; Flags: uninsdeletevalue
Root: HKCR; Subkey: "SimplePresenter.Playlist"; ValueType: string; ValueData: "SimplePresenter Playlist"; Flags: uninsdeletekey
Root: HKCR; Subkey: "SimplePresenter.Playlist\DefaultIcon"; ValueType: string; ValueData: "{app}\Logo.ico"; Flags: uninsdeletekey
Root: HKCR; Subkey: "SimplePresenter.Playlist\shell\open\command"; ValueType: string; ValueData: """{app}\SimplePresenter.exe"" ""%1"""; Flags: uninsdeletekey

[Icons]
Name: "{group}\SimplePresenter"; Filename: "{app}\SimplePresenter.exe"
Name: "{commondesktop}\SimplePresenter"; Filename: "{app}\SimplePresenter.exe"; Tasks: desktopicon

[Tasks]
Name: "desktopicon"; Description: "Create a &desktop icon"; GroupDescription: "Additional icons:"

[Run]
Filename: "{app}\SimplePresenter.exe"; Description: "Launch SimplePresenter"; Flags: nowait postinstall skipifsilent

[Code]
const
  AppUninstallKey = 'SOFTWARE\Microsoft\Windows\CurrentVersion\Uninstall\{8C4B7F6D-2D74-4B1E-9B86-1234567890AB}_is1';

function GetInstalledVersion: String;
begin
  Result := '';
  if not RegQueryStringValue(HKLM64, AppUninstallKey, 'DisplayVersion', Result) then
    RegQueryStringValue(HKLM32, AppUninstallKey, 'DisplayVersion', Result);
end;

function InitializeSetup: Boolean;
var
  OldVer, Msg: String;
begin
  Result := True;
  OldVer := GetInstalledVersion;
  if OldVer <> '' then
  begin
    if OldVer = '{#SetupSetting("AppVersion")}' then
      Msg := 'SimplePresenter ' + OldVer + ' is already installed.' + #13#10#13#10 + 'Do you want to reinstall it?'
    else
      Msg := 'SimplePresenter ' + OldVer + ' is currently installed.' + #13#10#13#10 + 'Do you want to update to version {#SetupSetting("AppVersion")}?';
    if MsgBox(Msg, mbConfirmation, MB_YESNO) = IDNO then
      Result := False;
  end;
end;
