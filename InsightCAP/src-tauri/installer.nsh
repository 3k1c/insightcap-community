; InsightCAP NSIS Uninstaller Hooks
; This file is used to customize the uninstallation process to include an option to delete user data.

!macro NSIS_HOOK_POSTINSTALL
  IfFileExists "$INSTDIR\resources\vc-runtime\*.dll" 0 +3
    DetailPrint "Installing Microsoft Visual C++ runtime files..."
    CopyFiles /SILENT "$INSTDIR\resources\vc-runtime\*.dll" "$INSTDIR"
!macroend

!macro NSIS_HOOK_PREUNINSTALL
  ; 問用戶是否刪除資料
  MessageBox MB_YESNO "是否要刪除 InsightCAP 的所有使用者資料？(這將會刪除您所有的本地知識庫、設定、緩存及恢復碼，且無法復原！)" IDNO +3
    RMDir /r "$APPDATA\com.insightcap.app"
    RMDir /r "$LOCALAPPDATA\com.insightcap.app"
    DetailPrint "已清理所有使用者資料"
!macroend
