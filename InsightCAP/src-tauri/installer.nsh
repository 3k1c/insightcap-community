; InsightCAP NSIS Installer/Uninstaller Hooks

!macro NSIS_HOOK_POSTINSTALL
  ; 先檢查系統是否已安裝 VC++ Runtime（檢查 System32 中的 msvcp140.dll）
  ; 若已存在則完全跳過，避免不必要的等待
  IfFileExists "$SYSDIR\msvcp140.dll" done_vc 0
    ; 系統尚未安裝 VC++ Runtime，嘗試靜默安裝
    IfFileExists "$INSTDIR\resources\vc_redist.x64.exe" run_redist try_copy_dlls

    run_redist:
      DetailPrint "Installing Microsoft Visual C++ Redistributable..."
      ExecWait '"$INSTDIR\resources\vc_redist.x64.exe" /quiet /norestart' $0
      DetailPrint "VC++ Redistributable installer exited with code: $0"
      Goto done_vc

    try_copy_dlls:
      ; 備用方案：直接複製 DLL 到安裝目錄
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

  ; 問用戶是否刪除資料
  MessageBox MB_YESNO "是否要刪除 InsightCAP 的所有使用者資料？(這將會刪除您所有的本地知識庫、設定、緩存及恢復碼，且無法復原！)" IDNO +3
    RMDir /r "$APPDATA\com.insightcap.app"
    RMDir /r "$LOCALAPPDATA\com.insightcap.app"
    DetailPrint "已清理所有使用者資料"
!macroend
