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

  MessageBox MB_YESNO "同時刪除 InsightCAP 本機 app 資料？$\r$\n$\r$\n這會刪除設定、快取、下載模型，以及預設 app 資料夾內的知識庫。$\r$\n不會刪除自定義知識庫資料夾，也不會清除 Windows Credential Manager 內的解密金鑰。" IDNO keep_app_data
    RMDir /r "$APPDATA\com.insightcap.app"
    RMDir /r "$LOCALAPPDATA\com.insightcap.app"
    DetailPrint "Deleted InsightCAP app-owned data."

  keep_app_data:
!macroend
