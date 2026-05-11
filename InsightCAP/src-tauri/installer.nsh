; InsightCAP NSIS Installer/Uninstaller Hooks

!macro NSIS_HOOK_POSTINSTALL
  ; Install VC++ Runtime only when the system runtime is missing.
  IfFileExists "$SYSDIR\msvcp140.dll" done_vc 0
  IfFileExists "$INSTDIR\resources\vc_redist.x64.exe" run_redist try_copy_dlls

  run_redist:
    DetailPrint "Installing Microsoft Visual C++ Redistributable..."
    ExecWait '"$INSTDIR\resources\vc_redist.x64.exe" /quiet /norestart' $0
    DetailPrint "VC++ Redistributable installer exited with code: $0"
    Goto done_vc

  try_copy_dlls:
    IfFileExists "$INSTDIR\resources\vc-runtime\*.dll" 0 done_vc
      DetailPrint "Copying Visual C++ runtime files..."
      CopyFiles /SILENT "$INSTDIR\resources\vc-runtime\*.dll" "$INSTDIR"

  done_vc:
    ; Refresh Windows shortcuts so Desktop and Start Menu use the current exe icon.
    IfFileExists "$INSTDIR\insightcap.exe" 0 refresh_icons_done
      Delete "$DESKTOP\InsightCAP.lnk"
      Delete "$SMPROGRAMS\InsightCAP.lnk"
      Delete "$SMPROGRAMS\InsightCAP\InsightCAP.lnk"

      CreateShortCut "$DESKTOP\InsightCAP.lnk" "$INSTDIR\insightcap.exe" "" "$INSTDIR\insightcap.exe" 0
      CreateShortCut "$SMPROGRAMS\InsightCAP.lnk" "$INSTDIR\insightcap.exe" "" "$INSTDIR\insightcap.exe" 0

      System::Call 'shell32::SHChangeNotify(i 0x08000000, i 0, i 0, i 0)'

    refresh_icons_done:
!macroend

!macro NSIS_HOOK_PREUNINSTALL
  Delete "$DESKTOP\InsightCAP.lnk"
  Delete "$SMPROGRAMS\InsightCAP.lnk"
  Delete "$SMPROGRAMS\InsightCAP\InsightCAP.lnk"

  ${If} $UpdateMode <> 1
    ${If} $InsightCAPClearKeysState = ${BST_CHECKED}
      nsExec::ExecToLog '"$SYSDIR\cmdkey.exe" /delete:auto_login_key.insightcap'
      nsExec::ExecToLog '"$SYSDIR\cmdkey.exe" /delete:recovery_pending_v1.insightcap'
      DetailPrint "Cleared InsightCAP Windows Credential Manager keys."
    ${EndIf}

    ${If} $InsightCAPDeleteCustomKbState = ${BST_CHECKED}
    ${AndIf} $InsightCAPCustomKbPath != ""
      IfFileExists "$InsightCAPCustomKbPath\.insightcap\insightcap.db" 0 insightcap_skip_custom_kb_delete
        RMDir /r "$InsightCAPCustomKbPath"
        DetailPrint "Deleted InsightCAP custom knowledge base: $InsightCAPCustomKbPath"
      insightcap_skip_custom_kb_delete:
    ${EndIf}
  ${EndIf}
!macroend
