ManifestDPIAware true
SetFont "Segoe UI" 9

!include WinMessages.nsh

!ifndef BUILD_UNINSTALLER
  Var allowUnsafeInstallation
  Var installationValidationAllowsUnsafe
  Var installationValidationDialog
  Var installationValidationMessage
  Var installationValidationRepairable
  Var installationValidationRepairButton
  Var installationValidationUnsafeButton
  Var unsafeInstallationAcknowledgementCheckbox
  Var unsafeInstallationConfirmationRequested
  Var unsafeInstallationReturnButton
  Var unsafeInstallationAncestor
  Var resetWorkingDirectory
  Var workingDirectory
!endif

!define MUI_CUSTOMFUNCTION_GUIINIT clearBrandingText
!ifdef BUILD_UNINSTALLER
  !define MUI_CUSTOMFUNCTION_UNGUIINIT un.clearBrandingText
!endif

!macro clearBrandingTextControl
  GetDlgItem $0 $HWNDPARENT 1256
  SendMessage $0 ${WM_SETTEXT} 0 "STR:"
!macroend

Function clearBrandingText
  !insertmacro clearBrandingTextControl
FunctionEnd

!ifdef BUILD_UNINSTALLER
  Function un.clearBrandingText
    !insertmacro clearBrandingTextControl
  FunctionEnd
!endif

!macro customHeader
  !insertmacro MUI_LANGUAGE "Farsi"

  LangString win7Required ${LANG_FARSI} "ویندوز ۷ یا بالاتر مورد نیاز است"
  LangString x64WinRequired ${LANG_FARSI} "ویندوز ۶۴ بیتی مورد نیاز است"
  LangString appRunning ${LANG_FARSI} "${PRODUCT_NAME} در حال اجرا است.$\r$\nبرای بستن آن روی «تأیید» کلیک کنید.$\r$\nاگر بسته نشد، آن را به‌صورت دستی ببندید."
  LangString appCannotBeClosed ${LANG_FARSI} "${PRODUCT_NAME} بسته نمی‌شود.$\r$\nلطفاً آن را به‌صورت دستی ببندید و برای ادامه روی «تلاش مجدد» کلیک کنید."
  LangString installing ${LANG_FARSI} "در حال نصب، لطفاً منتظر بمانید..."
  LangString areYouSureToUninstall ${LANG_FARSI} "آیا مطمئن هستید که می‌خواهید ${PRODUCT_NAME} را حذف کنید؟"
  LangString decompressionFailed ${LANG_FARSI} "استخراج فایل‌ها ناموفق بود. لطفاً نصب‌کننده را دوباره اجرا کنید."
  LangString uninstallFailed ${LANG_FARSI} "حذف فایل‌های نسخهٔ قبلی ناموفق بود. لطفاً نصب‌کننده را دوباره اجرا کنید."
  LangString appClosing ${LANG_FARSI} "در حال بستن ${PRODUCT_NAME}..."
  LangString chooseInstallationOptions ${LANG_FARSI} "انتخاب گزینه‌های نصب"
  LangString chooseUninstallationOptions ${LANG_FARSI} "انتخاب گزینه‌های حذف"
  LangString whichInstallationShouldBeRemoved ${LANG_FARSI} "کدام نصب باید حذف شود؟"
  LangString whoShouldThisApplicationBeInstalledFor ${LANG_FARSI} "این برنامه برای چه کسانی نصب شود؟"
  LangString selectUserMode ${LANG_FARSI} "انتخاب کنید این برنامه برای همهٔ کاربران یا فقط برای شما در دسترس باشد"
  LangString whichInstallationRemove ${LANG_FARSI} "این برنامه هم برای همهٔ کاربران و هم برای کاربر فعلی نصب شده است.$\r$\nکدام نصب را می‌خواهید حذف کنید؟"
  LangString freshInstallForAll ${LANG_FARSI} "نصب تازه برای همهٔ کاربران (نیازمند مجوز مدیر)"
  LangString freshInstallForCurrent ${LANG_FARSI} "نصب تازه فقط برای کاربر فعلی"
  LangString onlyForMe ${LANG_FARSI} "فقط برای &من"
  LangString forAll ${LANG_FARSI} "برای همهٔ کاربران این رایانه (&همه)"
  LangString loginWithAdminAccount ${LANG_FARSI} "برای ادامه باید با حسابی وارد شوید که عضو گروه مدیران است..."
  LangString perUserInstallExists ${LANG_FARSI} "یک نصب برای کاربر فعلی از قبل وجود دارد."
  LangString perUserInstall ${LANG_FARSI} "یک نصب برای کاربر فعلی وجود دارد."
  LangString perMachineInstallExists ${LANG_FARSI} "یک نصب برای همهٔ کاربران از قبل وجود دارد."
  LangString perMachineInstall ${LANG_FARSI} "یک نصب برای همهٔ کاربران وجود دارد."
  LangString reinstallUpgrade ${LANG_FARSI} "دوباره نصب یا ارتقا داده می‌شود."
  LangString uninstall ${LANG_FARSI} "حذف می‌شود."

  !ifdef BUILD_UNINSTALLER
    ShowUninstDetails show
  !else
    ShowInstDetails show
  !endif

  LangString keepData ${LANG_ENGLISH} "&Keep data"
  LangString keepData ${LANG_SIMPCHINESE} "保留数据"
  LangString keepData ${LANG_TRADCHINESE} "保留資料"
  LangString keepData ${LANG_FARSI} "&حفظ داده‌ها"
  LangString keepData ${LANG_RUSSIAN} "&Сохранить данные"

  LangString stoppingService ${LANG_ENGLISH} "Stopping the sing-box service..."
  LangString stoppingService ${LANG_SIMPCHINESE} "正在停止 sing-box 服务..."
  LangString stoppingService ${LANG_TRADCHINESE} "正在停止 sing-box 服務..."
  LangString stoppingService ${LANG_FARSI} "در حال توقف سرویس sing-box..."
  LangString stoppingService ${LANG_RUSSIAN} "Остановка службы sing-box..."

  LangString stopServiceFailed ${LANG_ENGLISH} "Failed to stop the sing-box service."
  LangString stopServiceFailed ${LANG_SIMPCHINESE} "无法停止 sing-box 服务。"
  LangString stopServiceFailed ${LANG_TRADCHINESE} "無法停止 sing-box 服務。"
  LangString stopServiceFailed ${LANG_FARSI} "توقف سرویس sing-box ناموفق بود."
  LangString stopServiceFailed ${LANG_RUSSIAN} "Не удалось остановить службу sing-box."

  LangString previousUninstallerFailed ${LANG_ENGLISH} "The previous uninstaller returned code $R0; continuing the upgrade."
  LangString previousUninstallerFailed ${LANG_SIMPCHINESE} "旧版本卸载程序返回代码 $R0；继续升级。"
  LangString previousUninstallerFailed ${LANG_TRADCHINESE} "舊版本解除安裝程式傳回代碼 $R0；繼續升級。"
  LangString previousUninstallerFailed ${LANG_FARSI} "حذف‌کنندهٔ نسخهٔ قبلی کد $R0 را برگرداند؛ ارتقا ادامه می‌یابد."
  LangString previousUninstallerFailed ${LANG_RUSSIAN} "Программа удаления предыдущей версии вернула код $R0; обновление будет продолжено."

  LangString architectureMismatchPrompt ${LANG_ENGLISH} "This installer contains the $0 build, but the native Windows architecture is $1.$\r$\n$\r$\nFor the best compatibility and performance, use the native installer. Continue anyway?"
  LangString architectureMismatchPrompt ${LANG_SIMPCHINESE} "此安装程序包含 $0 版本，但 Windows 的原生架构为 $1。$\r$\n$\r$\n为了获得最佳兼容性和性能，请改用原生安装程序。仍要继续吗？"
  LangString architectureMismatchPrompt ${LANG_TRADCHINESE} "此安裝程式包含 $0 版本，但 Windows 的原生架構為 $1。$\r$\n$\r$\n為了獲得最佳相容性與效能，請改用原生安裝程式。仍要繼續嗎？"
  LangString architectureMismatchPrompt ${LANG_FARSI} "این نصب‌کننده شامل نسخهٔ $0 است، اما معماری بومی ویندوز $1 است.$\r$\n$\r$\nبرای بهترین سازگاری و کارایی، از نصب‌کنندهٔ بومی استفاده کنید. با این حال ادامه می‌دهید؟"
  LangString architectureMismatchPrompt ${LANG_RUSSIAN} "Этот установщик содержит сборку $0, но собственная архитектура Windows — $1.$\r$\n$\r$\nДля лучшей совместимости и производительности используйте нативный установщик. Всё равно продолжить?"

  LangString unknownNativeArchitecture ${LANG_ENGLISH} "unknown"
  LangString unknownNativeArchitecture ${LANG_SIMPCHINESE} "未知"
  LangString unknownNativeArchitecture ${LANG_TRADCHINESE} "未知"
  LangString unknownNativeArchitecture ${LANG_FARSI} "ناشناخته"
  LangString unknownNativeArchitecture ${LANG_RUSSIAN} "неизвестна"

  LangString registeringService ${LANG_ENGLISH} "Registering the sing-box service..."
  LangString registeringService ${LANG_SIMPCHINESE} "正在注册 sing-box 服务..."
  LangString registeringService ${LANG_TRADCHINESE} "正在註冊 sing-box 服務..."
  LangString registeringService ${LANG_FARSI} "در حال ثبت سرویس sing-box..."
  LangString registeringService ${LANG_RUSSIAN} "Регистрация службы sing-box..."

  LangString registerServiceFailed ${LANG_ENGLISH} "Failed to register the sing-box service (code $1)."
  LangString registerServiceFailed ${LANG_SIMPCHINESE} "无法注册 sing-box 服务（代码 $1）。"
  LangString registerServiceFailed ${LANG_TRADCHINESE} "無法註冊 sing-box 服務（代碼 $1）。"
  LangString registerServiceFailed ${LANG_FARSI} "ثبت سرویس sing-box ناموفق بود (کد $1)."
  LangString registerServiceFailed ${LANG_RUSSIAN} "Не удалось зарегистрировать службу sing-box (код $1)."

  LangString serviceCommandNoDetails ${LANG_ENGLISH} "The service command produced no diagnostic output."
  LangString serviceCommandNoDetails ${LANG_SIMPCHINESE} "服务命令未产生诊断输出。"
  LangString serviceCommandNoDetails ${LANG_TRADCHINESE} "服務命令未產生診斷輸出。"
  LangString serviceCommandNoDetails ${LANG_FARSI} "فرمان سرویس هیچ خروجی عیب‌یابی ایجاد نکرد."
  LangString serviceCommandNoDetails ${LANG_RUSSIAN} "Команда службы не вывела диагностических данных."

  LangString copyingApplicationFiles ${LANG_ENGLISH} "Copying sing-box application files..."
  LangString copyingApplicationFiles ${LANG_SIMPCHINESE} "正在复制 sing-box 应用程序文件..."
  LangString copyingApplicationFiles ${LANG_TRADCHINESE} "正在複製 sing-box 應用程式檔案..."
  LangString copyingApplicationFiles ${LANG_FARSI} "در حال کپی فایل‌های برنامهٔ sing-box..."
  LangString copyingApplicationFiles ${LANG_RUSSIAN} "Копирование файлов приложения sing-box..."

  LangString rollingBackInstallation ${LANG_ENGLISH} "The service setup failed. Removing the installed application..."
  LangString rollingBackInstallation ${LANG_SIMPCHINESE} "服务设置失败，正在移除已安装的应用程序..."
  LangString rollingBackInstallation ${LANG_TRADCHINESE} "服務設定失敗，正在移除已安裝的應用程式..."
  LangString rollingBackInstallation ${LANG_FARSI} "راه‌اندازی سرویس ناموفق بود. برنامهٔ نصب‌شده در حال حذف است..."
  LangString rollingBackInstallation ${LANG_RUSSIAN} "Не удалось настроить службу. Удаление установленного приложения..."

  LangString registerServiceFailedRolledBack ${LANG_ENGLISH} "Failed to register the sing-box service (code $1). The installed application was removed.$\r$\n$\r$\n$3"
  LangString registerServiceFailedRolledBack ${LANG_SIMPCHINESE} "无法注册 sing-box 服务（代码 $1）。已移除安装的应用程序。$\r$\n$\r$\n$3"
  LangString registerServiceFailedRolledBack ${LANG_TRADCHINESE} "無法註冊 sing-box 服務（代碼 $1）。已移除安裝的應用程式。$\r$\n$\r$\n$3"
  LangString registerServiceFailedRolledBack ${LANG_FARSI} "ثبت سرویس sing-box ناموفق بود (کد $1). برنامهٔ نصب‌شده حذف شد.$\r$\n$\r$\n$3"
  LangString registerServiceFailedRolledBack ${LANG_RUSSIAN} "Не удалось зарегистрировать службу sing-box (код $1). Установленное приложение удалено.$\r$\n$\r$\n$3"

  LangString registerServiceFailedRollbackFailed ${LANG_ENGLISH} "Failed to register the sing-box service (code $1), and automatic removal failed (code $2).$\r$\n$\r$\n$3"
  LangString registerServiceFailedRollbackFailed ${LANG_SIMPCHINESE} "无法注册 sing-box 服务（代码 $1），且自动移除失败（代码 $2）。$\r$\n$\r$\n$3"
  LangString registerServiceFailedRollbackFailed ${LANG_TRADCHINESE} "無法註冊 sing-box 服務（代碼 $1），且自動移除失敗（代碼 $2）。$\r$\n$\r$\n$3"
  LangString registerServiceFailedRollbackFailed ${LANG_FARSI} "ثبت سرویس sing-box ناموفق بود (کد $1) و حذف خودکار نیز ناموفق بود (کد $2).$\r$\n$\r$\n$3"
  LangString registerServiceFailedRollbackFailed ${LANG_RUSSIAN} "Не удалось зарегистрировать службу sing-box (код $1), а автоматическое удаление завершилось ошибкой (код $2).$\r$\n$\r$\n$3"

  LangString unsafeInstallationDirectory ${LANG_ENGLISH} "The installation directory contains a reparse point or is not a normal directory:$\r$\n$INSTDIR"
  LangString unsafeInstallationDirectory ${LANG_SIMPCHINESE} "安装目录包含重解析点或不是普通目录：$\r$\n$INSTDIR"
  LangString unsafeInstallationDirectory ${LANG_TRADCHINESE} "安裝目錄包含重新解析點或不是一般目錄：$\r$\n$INSTDIR"
  LangString unsafeInstallationDirectory ${LANG_FARSI} "پوشهٔ نصب دارای نقطهٔ بازتحلیل است یا پوشهٔ عادی نیست:$\r$\n$INSTDIR"
  LangString unsafeInstallationDirectory ${LANG_RUSSIAN} "Каталог установки содержит точку повторной обработки или не является обычным каталогом:$\r$\n$INSTDIR"

  LangString invalidInstallationAncestor ${LANG_ENGLISH} "An installation path ancestor is not a normal directory or contains a reparse point:$\r$\n$0"
  LangString invalidInstallationAncestor ${LANG_SIMPCHINESE} "安装路径的上级目录不是普通目录或包含重解析点：$\r$\n$0"
  LangString invalidInstallationAncestor ${LANG_TRADCHINESE} "安裝路徑的上層目錄不是一般目錄或包含重新解析點：$\r$\n$0"
  LangString invalidInstallationAncestor ${LANG_FARSI} "یکی از پوشه‌های بالادستی مسیر نصب، پوشهٔ عادی نیست یا نقطهٔ بازتحلیل دارد:$\r$\n$0"
  LangString invalidInstallationAncestor ${LANG_RUSSIAN} "Родительский каталог пути установки не является обычным каталогом или содержит точку повторной обработки:$\r$\n$0"

  LangString unsafeInstallationPageTitle ${LANG_ENGLISH} "Unsafe installation path"
  LangString unsafeInstallationPageTitle ${LANG_SIMPCHINESE} "安装路径不安全"
  LangString unsafeInstallationPageTitle ${LANG_TRADCHINESE} "安裝路徑不安全"
  LangString unsafeInstallationPageTitle ${LANG_FARSI} "مسیر نصب ناامن"
  LangString unsafeInstallationPageTitle ${LANG_RUSSIAN} "Небезопасный путь установки"

  LangString unsafeInstallationPageSubtitle ${LANG_ENGLISH} "Repair the path when possible or continue with unsafe installation."
  LangString unsafeInstallationPageSubtitle ${LANG_SIMPCHINESE} "可修复时修复路径，或继续不安全安装。"
  LangString unsafeInstallationPageSubtitle ${LANG_TRADCHINESE} "可修復時修復路徑，或繼續不安全安裝。"
  LangString unsafeInstallationPageSubtitle ${LANG_FARSI} "در صورت امکان مسیر را اصلاح کنید یا نصب ناامن را ادامه دهید."
  LangString unsafeInstallationPageSubtitle ${LANG_RUSSIAN} "Исправьте путь, если это возможно, или продолжите небезопасную установку."

  LangString unsafeInstallationPageBody ${LANG_ENGLISH} "An unprivileged account can replace this installation path ancestor:$\r$\n$unsafeInstallationAncestor$\r$\n$\r$\nRepair removes only the dangerous permissions. Unsafe installation leaves the path unchanged."
  LangString unsafeInstallationPageBody ${LANG_SIMPCHINESE} "非管理员账户可以替换此安装路径的上级目录：$\r$\n$unsafeInstallationAncestor$\r$\n$\r$\n修复只会移除危险权限。不安全安装会保持此路径不变。"
  LangString unsafeInstallationPageBody ${LANG_TRADCHINESE} "非系統管理員帳戶可以取代此安裝路徑的上層目錄：$\r$\n$unsafeInstallationAncestor$\r$\n$\r$\n修復只會移除危險權限。不安全安裝會保持此路徑不變。"
  LangString unsafeInstallationPageBody ${LANG_FARSI} "یک حساب غیرمدیر می‌تواند این پوشهٔ بالادستی مسیر نصب را جایگزین کند:$\r$\n$unsafeInstallationAncestor$\r$\n$\r$\nاصلاح فقط مجوزهای خطرناک را حذف می‌کند. نصب ناامن مسیر را بدون تغییر نگه می‌دارد."
  LangString unsafeInstallationPageBody ${LANG_RUSSIAN} "Учетная запись без прав администратора может заменить этот родительский каталог:$\r$\n$unsafeInstallationAncestor$\r$\n$\r$\nИсправление удалит только опасные разрешения. Небезопасная установка оставит путь без изменений."

  LangString repairInstallationPermissionsButton ${LANG_ENGLISH} "&Repair permissions"
  LangString repairInstallationPermissionsButton ${LANG_SIMPCHINESE} "修复权限(&R)"
  LangString repairInstallationPermissionsButton ${LANG_TRADCHINESE} "修復權限(&R)"
  LangString repairInstallationPermissionsButton ${LANG_FARSI} "&اصلاح مجوزها"
  LangString repairInstallationPermissionsButton ${LANG_RUSSIAN} "&Исправить разрешения"

  LangString unsafeInstallationButton ${LANG_ENGLISH} "&Unsafe installation"
  LangString unsafeInstallationButton ${LANG_SIMPCHINESE} "不安全安装(&U)"
  LangString unsafeInstallationButton ${LANG_TRADCHINESE} "不安全安裝(&U)"
  LangString unsafeInstallationButton ${LANG_FARSI} "&نصب ناامن"
  LangString unsafeInstallationButton ${LANG_RUSSIAN} "&Небезопасная установка"

  LangString unsafeInstallationConfirmationTitle ${LANG_ENGLISH} "Confirm unsafe installation"
  LangString unsafeInstallationConfirmationTitle ${LANG_SIMPCHINESE} "确认不安全安装"
  LangString unsafeInstallationConfirmationTitle ${LANG_TRADCHINESE} "確認不安全安裝"
  LangString unsafeInstallationConfirmationTitle ${LANG_FARSI} "تأیید نصب ناامن"
  LangString unsafeInstallationConfirmationTitle ${LANG_RUSSIAN} "Подтверждение небезопасной установки"

  LangString unsafeInstallationConfirmationSubtitle ${LANG_ENGLISH} "Select the acknowledgement below to continue."
  LangString unsafeInstallationConfirmationSubtitle ${LANG_SIMPCHINESE} "必须勾选下方确认项才能继续。"
  LangString unsafeInstallationConfirmationSubtitle ${LANG_TRADCHINESE} "必須勾選下方確認項才能繼續。"
  LangString unsafeInstallationConfirmationSubtitle ${LANG_FARSI} "برای ادامه باید تأیید زیر را انتخاب کنید."
  LangString unsafeInstallationConfirmationSubtitle ${LANG_RUSSIAN} "Чтобы продолжить, установите флажок подтверждения ниже."

  LangString unsafeInstallationConfirmationWarning ${LANG_ENGLISH} "The installer and service will skip installation path security checks and permission hardening. This can allow another program to replace sing-box and elevate privileges to SYSTEM."
  LangString unsafeInstallationConfirmationWarning ${LANG_SIMPCHINESE} "安装程序和服务将跳过安装路径安全检查与权限加固。这可能允许其他程序替换 sing-box 并提权到 SYSTEM。"
  LangString unsafeInstallationConfirmationWarning ${LANG_TRADCHINESE} "安裝程式和服務將略過安裝路徑安全檢查與權限強化。這可能允許其他程式取代 sing-box 並將權限提升至 SYSTEM。"
  LangString unsafeInstallationConfirmationWarning ${LANG_FARSI} "نصب‌کننده و سرویس، بررسی امنیت مسیر نصب و ایمن‌سازی مجوزها را نادیده می‌گیرند. این کار ممکن است به برنامه‌ای دیگر اجازه دهد sing-box را جایگزین کند و سطح دسترسی را به SYSTEM برساند."
  LangString unsafeInstallationConfirmationWarning ${LANG_RUSSIAN} "Установщик и служба пропустят проверку безопасности пути и усиление разрешений. Другая программа сможет подменить sing-box и повысить привилегии до SYSTEM."

  LangString unsafeInstallationAcknowledgement ${LANG_ENGLISH} "I understand and accept this security risk."
  LangString unsafeInstallationAcknowledgement ${LANG_SIMPCHINESE} "我理解并接受此安全风险。"
  LangString unsafeInstallationAcknowledgement ${LANG_TRADCHINESE} "我理解並接受此安全風險。"
  LangString unsafeInstallationAcknowledgement ${LANG_FARSI} "این خطر امنیتی را درک می‌کنم و می‌پذیرم."
  LangString unsafeInstallationAcknowledgement ${LANG_RUSSIAN} "Я понимаю и принимаю этот риск безопасности."

  LangString continueUnsafeInstallationButton ${LANG_ENGLISH} "Continue unsafe installation"
  LangString continueUnsafeInstallationButton ${LANG_SIMPCHINESE} "继续不安全安装"
  LangString continueUnsafeInstallationButton ${LANG_TRADCHINESE} "繼續不安全安裝"
  LangString continueUnsafeInstallationButton ${LANG_FARSI} "ادامهٔ نصب ناامن"
  LangString continueUnsafeInstallationButton ${LANG_RUSSIAN} "Продолжить небезопасную установку"

  LangString returnToInstallationChoiceButton ${LANG_ENGLISH} "Return"
  LangString returnToInstallationChoiceButton ${LANG_SIMPCHINESE} "返回"
  LangString returnToInstallationChoiceButton ${LANG_TRADCHINESE} "返回"
  LangString returnToInstallationChoiceButton ${LANG_FARSI} "بازگشت"
  LangString returnToInstallationChoiceButton ${LANG_RUSSIAN} "Вернуться"

  LangString invalidInstallationPageTitle ${LANG_ENGLISH} "Unsupported installation path"
  LangString invalidInstallationPageTitle ${LANG_SIMPCHINESE} "不支持的安装路径"
  LangString invalidInstallationPageTitle ${LANG_TRADCHINESE} "不支援的安裝路徑"
  LangString invalidInstallationPageTitle ${LANG_FARSI} "مسیر نصب پشتیبانی‌نشده"
  LangString invalidInstallationPageTitle ${LANG_RUSSIAN} "Неподдерживаемый путь установки"

  LangString invalidInstallationPageSubtitle ${LANG_ENGLISH} "The installer cannot continue with this path."
  LangString invalidInstallationPageSubtitle ${LANG_SIMPCHINESE} "安装程序无法继续使用此路径。"
  LangString invalidInstallationPageSubtitle ${LANG_TRADCHINESE} "安裝程式無法繼續使用此路徑。"
  LangString invalidInstallationPageSubtitle ${LANG_FARSI} "نصب‌کننده نمی‌تواند با این مسیر ادامه دهد."
  LangString invalidInstallationPageSubtitle ${LANG_RUSSIAN} "Установщик не может продолжить с этим путем."

  LangString untrustedInstallationAncestor ${LANG_ENGLISH} "The installer cannot guarantee the permissions of this installation path ancestor:$\r$\n$0"
  LangString untrustedInstallationAncestor ${LANG_SIMPCHINESE} "安装程序无法保证此安装路径上级目录的权限：$\r$\n$0"
  LangString untrustedInstallationAncestor ${LANG_TRADCHINESE} "安裝程式無法保證此安裝路徑上層目錄的權限：$\r$\n$0"
  LangString untrustedInstallationAncestor ${LANG_FARSI} "نصب‌کننده نمی‌تواند مجوزهای این پوشهٔ بالادستی مسیر نصب را تضمین کند:$\r$\n$0"
  LangString untrustedInstallationAncestor ${LANG_RUSSIAN} "Установщик не может гарантировать разрешения этого родительского каталога:$\r$\n$0"

  LangString installationDriveNotFixed ${LANG_ENGLISH} "This path is not on a fixed local drive:$\r$\n$0"
  LangString installationDriveNotFixed ${LANG_SIMPCHINESE} "此路径不在固定的本地磁盘上：$\r$\n$0"
  LangString installationDriveNotFixed ${LANG_TRADCHINESE} "此路徑不在固定的本機磁碟上：$\r$\n$0"
  LangString installationDriveNotFixed ${LANG_FARSI} "این مسیر روی یک درایو محلی ثابت نیست:$\r$\n$0"
  LangString installationDriveNotFixed ${LANG_RUSSIAN} "Этот путь находится не на постоянном локальном диске:$\r$\n$0"

  LangString installationFileSystemNotNTFS ${LANG_ENGLISH} "This path uses the $0 file system instead of NTFS. Permission hardening cannot be guaranteed."
  LangString installationFileSystemNotNTFS ${LANG_SIMPCHINESE} "此路径使用 $0 文件系统，而不是 NTFS，无法保证权限加固。"
  LangString installationFileSystemNotNTFS ${LANG_TRADCHINESE} "此路徑使用 $0 檔案系統，而不是 NTFS，無法保證權限強化。"
  LangString installationFileSystemNotNTFS ${LANG_FARSI} "این مسیر به‌جای NTFS از فایل‌سیستم $0 استفاده می‌کند و ایمن‌سازی مجوزها تضمین نمی‌شود."
  LangString installationFileSystemNotNTFS ${LANG_RUSSIAN} "Этот путь использует файловую систему $0 вместо NTFS; усиление разрешений не гарантируется."

  LangString installationVolumeNotVerified ${LANG_ENGLISH} "The installer could not verify the installation volume:$\r$\n$0"
  LangString installationVolumeNotVerified ${LANG_SIMPCHINESE} "安装程序无法验证安装卷：$\r$\n$0"
  LangString installationVolumeNotVerified ${LANG_TRADCHINESE} "安裝程式無法驗證安裝磁碟區：$\r$\n$0"
  LangString installationVolumeNotVerified ${LANG_FARSI} "نصب‌کننده نتوانست درایو نصب را بررسی کند:$\r$\n$0"
  LangString installationVolumeNotVerified ${LANG_RUSSIAN} "Установщик не смог проверить том установки:$\r$\n$0"

  LangString repairInstallationAncestorFailed ${LANG_ENGLISH} "Could not repair the installation path permissions (code $1). No application files were installed."
  LangString repairInstallationAncestorFailed ${LANG_SIMPCHINESE} "无法修复安装路径权限（代码 $1）。尚未安装任何应用程序文件。"
  LangString repairInstallationAncestorFailed ${LANG_TRADCHINESE} "無法修復安裝路徑權限（代碼 $1）。尚未安裝任何應用程式檔案。"
  LangString repairInstallationAncestorFailed ${LANG_FARSI} "اصلاح مجوزهای مسیر نصب ممکن نشد (کد $1). هیچ فایل برنامه‌ای نصب نشد."
  LangString repairInstallationAncestorFailed ${LANG_RUSSIAN} "Не удалось исправить разрешения пути установки (код $1). Файлы приложения ещё не устанавливались."

  !ifndef BUILD_UNINSTALLER
  LangString resetWorkingDirectoryPrompt ${LANG_ENGLISH} "The sing-box service data directory is invalid:$\r$\n$workingDirectory$\r$\nContinuing will delete this directory so the service can recreate it securely. Continue?"
  LangString resetWorkingDirectoryPrompt ${LANG_SIMPCHINESE} "sing-box 服务数据目录无效：$\r$\n$workingDirectory$\r$\n继续安装将删除此目录，以便服务用安全权限重新创建。是否继续？"
  LangString resetWorkingDirectoryPrompt ${LANG_TRADCHINESE} "sing-box 服務資料目錄無效：$\r$\n$workingDirectory$\r$\n繼續安裝將刪除此目錄，以便服務用安全權限重新建立。是否繼續？"
  LangString resetWorkingDirectoryPrompt ${LANG_FARSI} "پوشهٔ دادهٔ سرویس sing-box نامعتبر است:$\r$\n$workingDirectory$\r$\nبا ادامه، این پوشه حذف می‌شود تا سرویس آن را با مجوزهای امن دوباره ایجاد کند. ادامه می‌دهید؟"
  LangString resetWorkingDirectoryPrompt ${LANG_RUSSIAN} "Каталог данных службы sing-box недействителен:$\r$\n$workingDirectory$\r$\nПри продолжении каталог будет удалён, чтобы служба могла безопасно создать его заново. Продолжить?"

  LangString resettingWorkingDirectory ${LANG_ENGLISH} "Resetting the sing-box service data directory..."
  LangString resettingWorkingDirectory ${LANG_SIMPCHINESE} "正在重置 sing-box 服务数据目录..."
  LangString resettingWorkingDirectory ${LANG_TRADCHINESE} "正在重設 sing-box 服務資料目錄..."
  LangString resettingWorkingDirectory ${LANG_FARSI} "در حال بازنشانی پوشهٔ دادهٔ سرویس sing-box..."
  LangString resettingWorkingDirectory ${LANG_RUSSIAN} "Сброс каталога данных службы sing-box..."

  LangString resetWorkingDirectoryFailed ${LANG_ENGLISH} "Could not reset the sing-box service data directory (code $1). No application files were installed."
  LangString resetWorkingDirectoryFailed ${LANG_SIMPCHINESE} "无法重置 sing-box 服务数据目录（代码 $1）。尚未安装任何应用程序文件。"
  LangString resetWorkingDirectoryFailed ${LANG_TRADCHINESE} "無法重設 sing-box 服務資料目錄（代碼 $1）。尚未安裝任何應用程式檔案。"
  LangString resetWorkingDirectoryFailed ${LANG_FARSI} "بازنشانی پوشهٔ دادهٔ سرویس sing-box ممکن نشد (کد $1). هیچ فایل برنامه‌ای نصب نشد."
  LangString resetWorkingDirectoryFailed ${LANG_RUSSIAN} "Не удалось сбросить каталог данных службы sing-box (код $1). Файлы приложения ещё не устанавливались."
  !endif

  LangString preflightFailed ${LANG_ENGLISH} "Could not validate the installation directories (code $1)."
  LangString preflightFailed ${LANG_SIMPCHINESE} "无法验证安装目录（代码 $1）。"
  LangString preflightFailed ${LANG_TRADCHINESE} "無法驗證安裝目錄（代碼 $1）。"
  LangString preflightFailed ${LANG_FARSI} "اعتبارسنجی پوشه‌های نصب ممکن نشد (کد $1)."
  LangString preflightFailed ${LANG_RUSSIAN} "Не удалось проверить каталоги установки (код $1)."

  LangString removingService ${LANG_ENGLISH} "Removing the sing-box service..."
  LangString removingService ${LANG_SIMPCHINESE} "正在移除 sing-box 服务..."
  LangString removingService ${LANG_TRADCHINESE} "正在移除 sing-box 服務..."
  LangString removingService ${LANG_FARSI} "در حال حذف سرویس sing-box..."
  LangString removingService ${LANG_RUSSIAN} "Удаление службы sing-box..."

  LangString removeServiceFailed ${LANG_ENGLISH} "Failed to remove the sing-box service (code $1).$\r$\n$\r$\n$3"
  LangString removeServiceFailed ${LANG_SIMPCHINESE} "无法移除 sing-box 服务（代码 $1）。$\r$\n$\r$\n$3"
  LangString removeServiceFailed ${LANG_TRADCHINESE} "無法移除 sing-box 服務（代碼 $1）。$\r$\n$\r$\n$3"
  LangString removeServiceFailed ${LANG_FARSI} "حذف سرویس sing-box ناموفق بود (کد $1).$\r$\n$\r$\n$3"
  LangString removeServiceFailed ${LANG_RUSSIAN} "Не удалось удалить службу sing-box (код $1).$\r$\n$\r$\n$3"

  LangString removingData ${LANG_ENGLISH} "Removing sing-box data..."
  LangString removingData ${LANG_SIMPCHINESE} "正在移除 sing-box 数据..."
  LangString removingData ${LANG_TRADCHINESE} "正在移除 sing-box 資料..."
  LangString removingData ${LANG_FARSI} "در حال حذف داده‌های sing-box..."
  LangString removingData ${LANG_RUSSIAN} "Удаление данных sing-box..."
!macroend

!macro daemonExecutable OUT
  StrCpy ${OUT} "$INSTDIR\resources\daemon\sing-box-daemon.exe"
!macroend

!macro executeDaemonServiceCommand ACTION OPTIONS RESULT DETAILS
  Delete "$PLUGINSDIR\service-command.log"
  StrCpy ${RESULT} -1
  StrCpy ${DETAILS} ""
  ClearErrors
  nsExec::Exec '"$SYSDIR\WindowsPowerShell\v1.0\powershell.exe" -NoProfile -NonInteractive -ExecutionPolicy Bypass -File "$PLUGINSDIR\installer-service.ps1" -ExecutablePath "$0" -ServiceAction ${ACTION} -OutputPath "$PLUGINSDIR\service-command.log" ${OPTIONS}'
  Pop ${RESULT}
  ${if} ${RESULT} == "error"
    StrCpy ${RESULT} -1
  ${endif}
  ClearErrors
  FileOpen $4 "$PLUGINSDIR\service-command.log" r
  ${ifNot} ${Errors}
    FileReadUTF16LE $4 ${DETAILS}
    FileClose $4
  ${endif}
  Delete "$PLUGINSDIR\service-command.log"
  ${if} ${RESULT} != 0
    ${if} ${DETAILS} == ""
      StrCpy ${DETAILS} "$(serviceCommandNoDetails)"
    ${endif}
    DetailPrint "${DETAILS}"
  ${endif}
!macroend

!macro confirmNativeArchitecture
  StrCpy $0 "x86"
  !ifdef APP_32
    StrCpy $0 "x86"
  !endif
  !ifdef APP_64
    StrCpy $0 "x64"
  !endif
  !ifdef APP_ARM64
    StrCpy $0 "ARM64"
  !endif
  StrCpy $1 "$(unknownNativeArchitecture)"
  ${if} ${IsNativeIA32}
    StrCpy $1 "x86"
  ${elseif} ${IsNativeAMD64}
    StrCpy $1 "x64"
  ${elseif} ${IsNativeARM64}
    StrCpy $1 "ARM64"
  ${endif}
  ${if} $0 != $1
    MessageBox MB_YESNO|MB_ICONEXCLAMATION|MB_DEFBUTTON2 "$(architectureMismatchPrompt)" /SD IDYES IDYES architectureMismatchAccepted IDNO architectureMismatchDeclined
    architectureMismatchDeclined:
      Abort
    architectureMismatchAccepted:
  ${endif}
!macroend

!macro customInit
  !insertmacro confirmNativeArchitecture
  StrCpy $allowUnsafeInstallation 0
  StrCpy $installationValidationAllowsUnsafe 0
  StrCpy $installationValidationMessage ""
  StrCpy $installationValidationRepairable 0
  StrCpy $unsafeInstallationConfirmationRequested 0
  StrCpy $unsafeInstallationAncestor ""
  StrCpy $resetWorkingDirectory 0
  SetShellVarContext all
  StrCpy $workingDirectory "$APPDATA\sing-box-daemon"
  InitPluginsDir
  File /oname=$PLUGINSDIR\installer-preflight.ps1 "${BUILD_RESOURCES_DIR}\installer-preflight.ps1"
  File /oname=$PLUGINSDIR\installer-service.ps1 "${BUILD_RESOURCES_DIR}\installer-service.ps1"
!macroend

!macro executeInstallationPreflight OPTIONS
  nsExec::ExecToStack '"$SYSDIR\WindowsPowerShell\v1.0\powershell.exe" -NoProfile -NonInteractive -ExecutionPolicy Bypass -File "$PLUGINSDIR\installer-preflight.ps1" -InstallationDirectory "$INSTDIR" ${OPTIONS}'
  Pop $1
  Pop $0
!macroend

!macro customPageAfterChangeDir
  Page custom showInstallationDirectoryValidationPage
  Page custom showUnsafeInstallationConfirmationPage

  Function showInstallationDirectoryValidationPage
    ${StrContains} $0 "${APP_FILENAME}" $INSTDIR
    ${if} $0 == ""
      StrCpy $INSTDIR "$INSTDIR\${APP_FILENAME}"
    ${endif}
    StrCpy $allowUnsafeInstallation 0
    StrCpy $installationValidationAllowsUnsafe 0
    StrCpy $installationValidationMessage ""
    StrCpy $installationValidationRepairable 0
    StrCpy $unsafeInstallationConfirmationRequested 0
    StrCpy $unsafeInstallationAncestor ""
    !insertmacro executeInstallationPreflight ""
    ${if} $1 == 0
      Abort
    ${elseif} $1 == 20
    ${orif} $1 == 21
    ${orif} $1 == 22
      MessageBox MB_YESNO|MB_ICONEXCLAMATION|MB_DEFBUTTON1 "$(resetWorkingDirectoryPrompt)" /SD IDYES IDYES acceptWorkingDirectoryReset IDNO declineWorkingDirectoryReset
      acceptWorkingDirectoryReset:
        StrCpy $resetWorkingDirectory 1
        Abort
      declineWorkingDirectoryReset:
        Quit
    ${endif}
    Call setInstallationValidationMessage
    ${if} $installationValidationAllowsUnsafe != 1
      !insertmacro MUI_HEADER_TEXT "$(invalidInstallationPageTitle)" "$(invalidInstallationPageSubtitle)"
    ${else}
      !insertmacro MUI_HEADER_TEXT "$(unsafeInstallationPageTitle)" "$(unsafeInstallationPageSubtitle)"
    ${endif}
    nsDialogs::Create 1018
    Pop $installationValidationDialog
    ${if} $installationValidationDialog == error
      Abort
    ${endif}

    ${NSD_CreateLabel} 0u 0u 100% 84u "$installationValidationMessage"
    Pop $0

    ${if} $installationValidationAllowsUnsafe == 1
      ${if} $installationValidationRepairable == 1
        ${NSD_CreateButton} 0u 90u 100% 18u "$(repairInstallationPermissionsButton)"
        Pop $installationValidationRepairButton
        ${NSD_OnClick} $installationValidationRepairButton repairInstallationPermissions

        ${NSD_CreateButton} 0u 112u 100% 18u "$(unsafeInstallationButton)"
        Pop $installationValidationUnsafeButton
        ${NSD_OnClick} $installationValidationUnsafeButton openUnsafeInstallationConfirmation
      ${else}
        ${NSD_CreateButton} 0u 90u 100% 18u "$(unsafeInstallationButton)"
        Pop $installationValidationUnsafeButton
        ${NSD_OnClick} $installationValidationUnsafeButton openUnsafeInstallationConfirmation
      ${endif}
    ${endif}

    GetDlgItem $0 $HWNDPARENT 1
    ShowWindow $0 ${SW_HIDE}
    GetDlgItem $0 $HWNDPARENT 2
    ShowWindow $0 ${SW_SHOW}
    ${if} $installationValidationRepairable == 1
      SendMessage $installationValidationDialog ${WM_NEXTDLGCTL} $installationValidationRepairButton 1
    ${else}
      SendMessage $installationValidationDialog ${WM_NEXTDLGCTL} $0 1
    ${endif}
    GetDlgItem $0 $HWNDPARENT 3
    ShowWindow $0 ${SW_SHOW}
    nsDialogs::Show
  FunctionEnd

  Function showUnsafeInstallationConfirmationPage
    ${if} $installationValidationAllowsUnsafe != 1
    ${orif} $unsafeInstallationConfirmationRequested != 1
      Abort
    ${endif}
    !insertmacro MUI_HEADER_TEXT "$(unsafeInstallationConfirmationTitle)" "$(unsafeInstallationConfirmationSubtitle)"
    nsDialogs::Create 1018
    Pop $installationValidationDialog
    ${if} $installationValidationDialog == error
      Abort
    ${endif}

    ${NSD_CreateLabel} 0u 0u 100% 32u "$installationValidationMessage"
    Pop $0

    ${NSD_CreateLabel} 0u 34u 100% 35u "$(unsafeInstallationConfirmationWarning)"
    Pop $0
    SetCtlColors $0 "C42B1C" transparent

    ${NSD_CreateCheckBox} 0u 73u 100% 12u "$(unsafeInstallationAcknowledgement)"
    Pop $unsafeInstallationAcknowledgementCheckbox
    ${NSD_SetState} $unsafeInstallationAcknowledgementCheckbox ${BST_UNCHECKED}
    ${NSD_OnClick} $unsafeInstallationAcknowledgementCheckbox updateUnsafeInstallationAcknowledgement

    ${NSD_CreateButton} 0u 91u 100% 18u "$(continueUnsafeInstallationButton)"
    Pop $installationValidationUnsafeButton
    ${NSD_OnClick} $installationValidationUnsafeButton continueUnsafeInstallation
    EnableWindow $installationValidationUnsafeButton 0

    ${NSD_CreateButton} 0u 113u 100% 18u "$(returnToInstallationChoiceButton)"
    Pop $unsafeInstallationReturnButton
    ${NSD_OnClick} $unsafeInstallationReturnButton returnToInstallationChoice

    GetDlgItem $0 $HWNDPARENT 1
    ShowWindow $0 ${SW_HIDE}
    GetDlgItem $0 $HWNDPARENT 2
    ShowWindow $0 ${SW_HIDE}
    GetDlgItem $0 $HWNDPARENT 3
    ShowWindow $0 ${SW_HIDE}
    SendMessage $installationValidationDialog ${WM_NEXTDLGCTL} $unsafeInstallationReturnButton 1
    nsDialogs::Show
  FunctionEnd

  Function setInstallationValidationMessage
    ${if} $1 == 10
      StrCpy $installationValidationAllowsUnsafe 1
      StrCpy $installationValidationMessage "$(unsafeInstallationDirectory)"
    ${elseif} $1 == 11
      StrCpy $installationValidationAllowsUnsafe 1
      StrCpy $installationValidationMessage "$(invalidInstallationAncestor)"
    ${elseif} $1 == 12
      StrCpy $installationValidationAllowsUnsafe 1
      StrCpy $installationValidationMessage "$(untrustedInstallationAncestor)"
    ${elseif} $1 == 13
      StrCpy $installationValidationAllowsUnsafe 1
      StrCpy $installationValidationRepairable 1
      StrCpy $unsafeInstallationAncestor "$0"
      StrCpy $installationValidationMessage "$(unsafeInstallationPageBody)"
    ${elseif} $1 == 14
      StrCpy $installationValidationAllowsUnsafe 1
      StrCpy $installationValidationMessage "$(installationDriveNotFixed)"
    ${elseif} $1 == 15
      StrCpy $installationValidationAllowsUnsafe 1
      StrCpy $installationValidationMessage "$(installationFileSystemNotNTFS)"
    ${elseif} $1 == 16
      StrCpy $installationValidationAllowsUnsafe 1
      StrCpy $installationValidationMessage "$(installationVolumeNotVerified)"
    ${else}
      StrCpy $installationValidationMessage "$(preflightFailed)"
    ${endif}
  FunctionEnd

  Function openUnsafeInstallationConfirmation
    Pop $0
    StrCpy $unsafeInstallationConfirmationRequested 1
    Call restoreInstallerNavigation
    SendMessage $HWNDPARENT ${WM_COMMAND} 1 0
  FunctionEnd

  Function returnToInstallationChoice
    Pop $0
    StrCpy $unsafeInstallationConfirmationRequested 0
    Call restoreInstallerNavigation
    SendMessage $HWNDPARENT ${WM_COMMAND} 3 0
  FunctionEnd

  Function continueUnsafeInstallation
    Pop $0
    ${NSD_GetState} $unsafeInstallationAcknowledgementCheckbox $0
    ${if} $0 != ${BST_CHECKED}
      Return
    ${endif}
    StrCpy $allowUnsafeInstallation 1
    !insertmacro executeInstallationPreflight "-AllowUnsafeInstallationDirectory"
    ${if} $1 == 0
      Call advanceInstallationDirectoryValidationPage
    ${elseif} $1 == 20
    ${orif} $1 == 21
    ${orif} $1 == 22
      MessageBox MB_YESNO|MB_ICONEXCLAMATION|MB_DEFBUTTON1 "$(resetWorkingDirectoryPrompt)" IDYES acceptWorkingDirectoryResetAfterUnsafeChoice IDNO declineWorkingDirectoryResetAfterUnsafeChoice
      acceptWorkingDirectoryResetAfterUnsafeChoice:
        StrCpy $resetWorkingDirectory 1
        Call advanceInstallationDirectoryValidationPage
        Return
      declineWorkingDirectoryResetAfterUnsafeChoice:
        Return
    ${else}
      StrCpy $allowUnsafeInstallation 0
      Call setInstallationValidationMessage
      MessageBox MB_OK|MB_ICONSTOP "$installationValidationMessage"
    ${endif}
  FunctionEnd

  Function updateUnsafeInstallationAcknowledgement
    Pop $0
    ${NSD_GetState} $unsafeInstallationAcknowledgementCheckbox $0
    ${if} $0 == ${BST_CHECKED}
      EnableWindow $installationValidationUnsafeButton 1
    ${else}
      EnableWindow $installationValidationUnsafeButton 0
    ${endif}
  FunctionEnd

  Function repairInstallationPermissions
    Pop $0
    !insertmacro executeInstallationPreflight "-RepairInstallationAncestors"
    ${if} $1 == 0
      StrCpy $allowUnsafeInstallation 0
      Call advanceInstallationDirectoryValidationPage
    ${elseif} $1 == 20
    ${orif} $1 == 21
    ${orif} $1 == 22
      MessageBox MB_YESNO|MB_ICONEXCLAMATION|MB_DEFBUTTON1 "$(resetWorkingDirectoryPrompt)" IDYES acceptWorkingDirectoryResetAfterRepair IDNO declineWorkingDirectoryResetAfterRepair
      acceptWorkingDirectoryResetAfterRepair:
        StrCpy $resetWorkingDirectory 1
        StrCpy $allowUnsafeInstallation 0
        Call advanceInstallationDirectoryValidationPage
        Return
      declineWorkingDirectoryResetAfterRepair:
        Return
    ${elseif} $1 == 32
      MessageBox MB_OK|MB_ICONSTOP "$(repairInstallationAncestorFailed)"
    ${else}
      Call setInstallationValidationMessage
      MessageBox MB_OK|MB_ICONSTOP "$installationValidationMessage"
    ${endif}
  FunctionEnd

  Function restoreInstallerNavigation
    GetDlgItem $0 $HWNDPARENT 1
    ShowWindow $0 ${SW_SHOW}
    GetDlgItem $0 $HWNDPARENT 2
    ShowWindow $0 ${SW_SHOW}
    GetDlgItem $0 $HWNDPARENT 3
    ShowWindow $0 ${SW_SHOW}
  FunctionEnd

  Function advanceInstallationDirectoryValidationPage
    StrCpy $unsafeInstallationConfirmationRequested 0
    Call restoreInstallerNavigation
    SendMessage $HWNDPARENT ${WM_COMMAND} 1 0
  FunctionEnd
!macroend

!macro customInstallBeforeApplicationFiles
  SetDetailsPrint both
  ${if} $resetWorkingDirectory == 1
    DetailPrint "$(resettingWorkingDirectory)"
    ${if} $allowUnsafeInstallation == 1
      nsExec::ExecToLog '"$SYSDIR\WindowsPowerShell\v1.0\powershell.exe" -NoProfile -NonInteractive -ExecutionPolicy Bypass -File "$PLUGINSDIR\installer-preflight.ps1" -InstallationDirectory "$INSTDIR" -AllowUnsafeInstallationDirectory -ResetWorkingDirectory'
    ${else}
      nsExec::ExecToLog '"$SYSDIR\WindowsPowerShell\v1.0\powershell.exe" -NoProfile -NonInteractive -ExecutionPolicy Bypass -File "$PLUGINSDIR\installer-preflight.ps1" -InstallationDirectory "$INSTDIR" -ResetWorkingDirectory'
    ${endif}
    Pop $1
    ${if} $1 != 0
      Abort "$(resetWorkingDirectoryFailed)"
    ${endif}
  ${endif}
  validateInstallationDirectory:
  ${if} $allowUnsafeInstallation == 1
    !insertmacro executeInstallationPreflight "-AllowUnsafeInstallationDirectory"
  ${else}
    !insertmacro executeInstallationPreflight ""
  ${endif}
  ${if} $1 == 13
    ${if} ${Silent}
      !insertmacro executeInstallationPreflight "-RepairInstallationAncestors"
      ${if} $1 == 0
        Goto validateInstallationDirectory
      ${elseif} $1 != 20
      ${andif} $1 != 21
      ${andif} $1 != 22
        Abort "$(repairInstallationAncestorFailed)"
      ${endif}
    ${endif}
  ${endif}
  ${if} $1 == 20
  ${orif} $1 == 21
  ${orif} $1 == 22
    ${if} ${Silent}
      DetailPrint "$(resettingWorkingDirectory)"
      nsExec::ExecToLog '"$SYSDIR\WindowsPowerShell\v1.0\powershell.exe" -NoProfile -NonInteractive -ExecutionPolicy Bypass -File "$PLUGINSDIR\installer-preflight.ps1" -InstallationDirectory "$INSTDIR" -ResetWorkingDirectory'
      Pop $1
      ${if} $1 != 0
        Abort "$(resetWorkingDirectoryFailed)"
      ${endif}
      Goto validateInstallationDirectory
    ${endif}
  ${endif}
  ${if} $1 != 0
    Call setInstallationValidationMessage
    Abort "$installationValidationMessage"
  ${endif}
  DetailPrint "$(stoppingService)"
  nsExec::ExecToLog '"$SYSDIR\WindowsPowerShell\v1.0\powershell.exe" -NoProfile -NonInteractive -Command "if (Get-Service -Name sing-box-daemon -ErrorAction SilentlyContinue) { Stop-Service -Name sing-box-daemon -Force -ErrorAction Stop; (Get-Service -Name sing-box-daemon).WaitForStatus([System.ServiceProcess.ServiceControllerStatus]::Stopped, [TimeSpan]::FromSeconds(10)) }"'
  Pop $1
  ${if} $1 != 0
    Abort "$(stopServiceFailed)"
  ${endif}
  DetailPrint "$(copyingApplicationFiles)"
!macroend

!macro customUnInstallCheck
  ${if} $R0 != 0
    DetailPrint "$(previousUninstallerFailed)"
  ${endif}
!macroend

!macro customInstall
  !insertmacro daemonExecutable $0
  SetDetailsPrint both
  DetailPrint "$(registeringService)"
  ${if} $allowUnsafeInstallation == 1
    !insertmacro executeDaemonServiceCommand "install" "-AllowUnsafeInstallationDirectoryPermissions" $1 $3
  ${else}
    !insertmacro executeDaemonServiceCommand "install" "" $1 $3
  ${endif}
  ${if} $1 != 0
    DetailPrint "$(rollingBackInstallation)"
    StrCpy $2 -1
    ClearErrors
    ExecWait '"$INSTDIR\${UNINSTALL_FILENAME}" /allusers /S' $2
    ${if} ${Errors}
      StrCpy $2 -1
    ${endif}
    ${if} $2 == 0
      Abort "$(registerServiceFailedRolledBack)"
    ${else}
      Abort "$(registerServiceFailedRollbackFailed)"
    ${endif}
  ${endif}
!macroend

!macro customUnWelcomePage
  Var keepUninstallData
  Var keepUninstallDataCheckbox

  !define MUI_PAGE_CUSTOMFUNCTION_SHOW un.showKeepDataCheckbox
  !define MUI_PAGE_CUSTOMFUNCTION_LEAVE un.readKeepDataCheckbox
  !insertmacro MUI_UNPAGE_WELCOME

  Function un.showKeepDataCheckbox
    ${NSD_CreateCheckBox} 120u 175u 195u 12u "$(keepData)"
    Pop $keepUninstallDataCheckbox
    ${NSD_SetState} $keepUninstallDataCheckbox $keepUninstallData
    SetCtlColors $keepUninstallDataCheckbox "${MUI_TEXTCOLOR}" "${MUI_BGCOLOR}"
  FunctionEnd

  Function un.readKeepDataCheckbox
    ${NSD_GetState} $keepUninstallDataCheckbox $keepUninstallData
  FunctionEnd
!macroend

!macro customUnInit
  StrCpy $keepUninstallData ${BST_CHECKED}
  InitPluginsDir
  File /oname=$PLUGINSDIR\installer-service.ps1 "${BUILD_RESOURCES_DIR}\installer-service.ps1"
  ${GetParameters} $R0
  ${GetOptions} $R0 "--delete-app-data" $R1
  ${ifNot} ${Errors}
    StrCpy $keepUninstallData ${BST_UNCHECKED}
  ${endif}
!macroend

!macro customUnInstall
  SetDetailsPrint both
  ${ifNot} ${isUpdated}
    !insertmacro daemonExecutable $0
    ${if} ${FileExists} "$0"
      DetailPrint "$(removingService)"
      !insertmacro executeDaemonServiceCommand "uninstall" "" $1 $3
      ${if} $1 != 0
        Abort "$(removeServiceFailed)"
      ${endif}
    ${endif}
    ${if} $keepUninstallData == ${BST_UNCHECKED}
      DetailPrint "$(removingData)"
      SetShellVarContext all
      RMDir /r "$APPDATA\sing-box-daemon"
      SetShellVarContext current
      RMDir /r "$APPDATA\sing-box"
      SetShellVarContext all
    ${endif}
  ${endIf}
!macroend
