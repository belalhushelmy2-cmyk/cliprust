Unicode true
!include "MUI2.nsh"
!include "x64.nsh"
!ifndef VERSION
!define VERSION "0.1.0"
!endif
!ifndef PAYLOAD
!define PAYLOAD "..\..\target\release"
!endif
!ifndef OUTPUT
!define OUTPUT "..\..\dist\Cliprust-${VERSION}-windows-x86_64-Setup.exe"
!endif
Name "Cliprust"
OutFile "${OUTPUT}"
InstallDir "$LOCALAPPDATA\Programs\Cliprust"
RequestExecutionLevel user
SetCompressor /SOLID lzma
VIProductVersion "${VERSION}.0"
VIAddVersionKey "ProductName" "Cliprust"
VIAddVersionKey "FileDescription" "Cliprust per-user installer"
VIAddVersionKey "FileVersion" "${VERSION}"
VIAddVersionKey "LegalCopyright" "Cliprust contributors"
!define MUI_ABORTWARNING
!insertmacro MUI_PAGE_WELCOME
!insertmacro MUI_PAGE_DIRECTORY
!insertmacro MUI_PAGE_INSTFILES
!define MUI_FINISHPAGE_RUN "$INSTDIR\cliprust.exe"
!define MUI_FINISHPAGE_RUN_TEXT "Start Cliprust"
!insertmacro MUI_PAGE_FINISH
!insertmacro MUI_UNPAGE_CONFIRM
!insertmacro MUI_UNPAGE_INSTFILES
!insertmacro MUI_LANGUAGE "English"
Function .onInit
  ${IfNot} ${RunningX64}
    MessageBox MB_OK|MB_ICONSTOP "This installer requires 64-bit Windows."
    Abort
  ${EndIf}
  SetRegView 64
  SetShellVarContext current
  ReadRegStr $0 HKCU "Software\Cliprust" "InstallDir"
  StrCmp $0 "" +2
  StrCpy $INSTDIR $0
FunctionEnd
Section "Cliprust"
  SetRegView 64
  SetShellVarContext current
  SetOutPath "$INSTDIR"
  ; File overwrite failures abort installation; quit the running app before upgrades.
  SetOverwrite on
  ClearErrors
  File /r "${PAYLOAD}\*.*"
  IfErrors 0 +3
    MessageBox MB_OK|MB_ICONSTOP "Installation was incomplete. Quit Cliprust and retry."
    Abort
  CreateShortcut "$SMPROGRAMS\Cliprust.lnk" "$INSTDIR\cliprust.exe"
  WriteUninstaller "$INSTDIR\Uninstall.exe"
  WriteRegStr HKCU "Software\Cliprust" "InstallDir" "$INSTDIR"
  WriteRegStr HKCU "Software\Microsoft\Windows\CurrentVersion\Uninstall\Cliprust" "DisplayName" "Cliprust"
  WriteRegStr HKCU "Software\Microsoft\Windows\CurrentVersion\Uninstall\Cliprust" "DisplayVersion" "${VERSION}"
  WriteRegStr HKCU "Software\Microsoft\Windows\CurrentVersion\Uninstall\Cliprust" "InstallLocation" "$INSTDIR"
  WriteRegStr HKCU "Software\Microsoft\Windows\CurrentVersion\Uninstall\Cliprust" "DisplayIcon" "$INSTDIR\cliprust.exe"
  WriteRegStr HKCU "Software\Microsoft\Windows\CurrentVersion\Uninstall\Cliprust" "UninstallString" '"$INSTDIR\Uninstall.exe"'
  WriteRegStr HKCU "Software\Microsoft\Windows\CurrentVersion\Uninstall\Cliprust" "QuietUninstallString" '"$INSTDIR\Uninstall.exe" /S'
  WriteRegDWORD HKCU "Software\Microsoft\Windows\CurrentVersion\Uninstall\Cliprust" "NoModify" 1
  WriteRegDWORD HKCU "Software\Microsoft\Windows\CurrentVersion\Uninstall\Cliprust" "NoRepair" 1
SectionEnd
Section "Uninstall"
  SetRegView 64
  SetShellVarContext current
  ClearErrors
  Delete "$INSTDIR\cliprust.exe"
  IfErrors 0 +3
    MessageBox MB_OK|MB_ICONSTOP "Quit Cliprust before uninstalling."
    Abort
  Delete "$SMPROGRAMS\Cliprust.lnk"
  Delete "$INSTDIR\libgcc_s_seh-1.dll"
  Delete "$INSTDIR\libstdc++-6.dll"
  Delete "$INSTDIR\libwinpthread-1.dll"
  Delete "$INSTDIR\README.md"
  Delete "$INSTDIR\TEST_RESULTS.md"
  Delete "$INSTDIR\LICENSE"
  Delete "$INSTDIR\THIRD-PARTY-LICENSES.txt"
  Delete "$INSTDIR\THIRD-PARTY-NOTICES.txt"
  Delete "$INSTDIR\Uninstall.exe"
  RMDir "$INSTDIR"
  ReadRegStr $0 HKCU "Software\Microsoft\Windows\CurrentVersion\Run" "Cliprust"
  StrCmp $0 '"$INSTDIR\cliprust.exe" --show' 0 +2
    DeleteRegValue HKCU "Software\Microsoft\Windows\CurrentVersion\Run" "Cliprust"
  DeleteRegKey HKCU "Software\Cliprust"
  DeleteRegKey HKCU "Software\Microsoft\Windows\CurrentVersion\Uninstall\Cliprust"
  ; Clipboard history in AppData is intentionally preserved.
SectionEnd
