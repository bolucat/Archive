export type DesktopLanguage = "en" | "zh-Hans" | "zh-Hant" | "fa" | "ru";

type DesktopMessage = Record<Exclude<DesktopLanguage, "en">, string>;

export const DESKTOP_TRANSLATIONS = {
  "Start": {
    "zh-Hans": "启动",
    "zh-Hant": "啟動",
    "fa": "شروع",
    "ru": "Запустить",
  },
  "Stop": {
    "zh-Hans": "停止",
    "zh-Hant": "停止",
    "fa": "توقف",
    "ru": "Остановить",
  },
  "Service": {
    "zh-Hans": "服务",
    "zh-Hant": "服務",
    "fa": "سرویس",
    "ru": "Служба",
  },
  "System HTTP Proxy": {
    "zh-Hans": "系统 HTTP 代理",
    "zh-Hant": "系統 HTTP 代理",
    "fa": "پروکسی HTTP سیستم",
    "ru": "Системный HTTP-прокси",
  },
  "Enabled": {
    "zh-Hans": "已启用",
    "zh-Hant": "已啟用",
    "fa": "فعال",
    "ru": "Включено",
  },
  "This Computer": {
    "zh-Hans": "本机",
    "zh-Hant": "本機",
    "fa": "این رایانه",
    "ru": "Этот компьютер",
  },
  "Profiles": {
    "zh-Hans": "配置",
    "zh-Hant": "配置",
    "fa": "پیکربندی",
    "ru": "Конфигурация",
  },
  "Profile": {
    "zh-Hans": "配置",
    "zh-Hant": "配置",
    "fa": "پیکربندی",
    "ru": "Конфигурация",
  },
  "New Profile": {
    "zh-Hans": "新建配置",
    "zh-Hant": "新建配置",
    "fa": "پیکربندی جدید",
    "ru": "Новая конфигурация",
  },
  "Name": {
    "zh-Hans": "名称",
    "zh-Hant": "名稱",
    "fa": "نام",
    "ru": "Имя",
  },
  "Type": {
    "zh-Hans": "类型",
    "zh-Hant": "類型",
    "fa": "نوع",
    "ru": "Тип",
  },
  "Local": {
    "zh-Hans": "本地",
    "zh-Hant": "本地",
    "fa": "محلی",
    "ru": "Локальный",
  },
  "Remote": {
    "zh-Hans": "远程",
    "zh-Hant": "遠程",
    "fa": "راه‌دور",
    "ru": "Удаленный",
  },
  "URL": {
    "zh-Hans": "URL",
    "zh-Hant": "URL",
    "fa": "URL",
    "ru": "URL",
  },
  "Auto Update": {
    "zh-Hans": "自动更新",
    "zh-Hant": "自動更新",
    "fa": "به‌روزرسانی خودکار",
    "ru": "Автообновление",
  },
  "Auto Update Interval": {
    "zh-Hans": "自动更新间隔",
    "zh-Hant": "自動更新間隔",
    "fa": "بازه به‌روزرسانی خودکار",
    "ru": "Интервал автообновления",
  },
  "Last Updated": {
    "zh-Hans": "最后更新",
    "zh-Hant": "最後更新",
    "fa": "آخرین به‌روزرسانی",
    "ru": "Последнее обновление",
  },
  "Edit": {
    "zh-Hans": "编辑",
    "zh-Hant": "編輯",
    "fa": "ویرایش",
    "ru": "Изменить",
  },
  "Rename": {
    "zh-Hans": "重命名",
    "zh-Hant": "重新命名",
    "fa": "تغییر نام",
    "ru": "Переименовать",
  },
  "Delete": {
    "zh-Hans": "删除",
    "zh-Hant": "刪除",
    "fa": "حذف",
    "ru": "Удалить",
  },
  "Share": {
    "zh-Hans": "分享",
    "zh-Hant": "分享",
    "fa": "اشتراک‌گذاری",
    "ru": "Поделиться",
  },
  "Import": {
    "zh-Hans": "导入",
    "zh-Hant": "導入",
    "fa": "وارد کردن",
    "ru": "Импорт",
  },
  "Export": {
    "zh-Hans": "导出",
    "zh-Hant": "導出",
    "fa": "خروجی گرفتن",
    "ru": "Экспорт",
  },
  "Check": {
    "zh-Hans": "检查",
    "zh-Hant": "檢查",
    "fa": "بررسی",
    "ru": "Проверить",
  },
  "Format": {
    "zh-Hans": "格式化",
    "zh-Hant": "格式化",
    "fa": "قالب‌بندی",
    "ru": "Форматировать",
  },
  "Save": {
    "zh-Hans": "保存",
    "zh-Hant": "保存",
    "fa": "ذخیره",
    "ru": "Сохранить",
  },
  "Update": {
    "zh-Hans": "更新",
    "zh-Hant": "更新",
    "fa": "به‌روزرسانی",
    "ru": "Обновить",
  },
  "Create": {
    "zh-Hans": "创建",
    "zh-Hant": "創建",
    "fa": "ایجاد",
    "ru": "Создать",
  },
  "Cancel": {
    "zh-Hans": "取消",
    "zh-Hant": "取消",
    "fa": "لغو",
    "ru": "Отмена",
  },
  "Move Up": {
    "zh-Hans": "上移",
    "zh-Hant": "上移",
    "fa": "انتقال به بالا",
    "ru": "Вверх",
  },
  "Move Down": {
    "zh-Hans": "下移",
    "zh-Hant": "下移",
    "fa": "انتقال به پایین",
    "ru": "Вниз",
  },
  "Done": {
    "zh-Hans": "完成",
    "zh-Hant": "完成",
    "fa": "انجام شد",
    "ru": "Готово",
  },
  "No profiles": {
    "zh-Hans": "没有配置",
    "zh-Hant": "沒有配置",
    "fa": "پیکربندی‌ای وجود ندارد",
    "ru": "Нет конфигураций",
  },
  "No profile selected": {
    "zh-Hans": "未选择配置",
    "zh-Hant": "未選擇配置",
    "fa": "پیکربندی انتخاب نشده است",
    "ru": "Конфигурация не выбрана",
  },
  "Open": {
    "zh-Hans": "打开",
    "zh-Hant": "打開",
    "fa": "باز کردن",
    "ru": "Открыть",
  },
  "Quit": {
    "zh-Hans": "退出",
    "zh-Hant": "退出",
    "fa": "خروج",
    "ru": "Выйти",
  },
  "Group": {
    "zh-Hans": "分组",
    "zh-Hant": "分組",
    "fa": "گروه",
    "ru": "Группа",
  },
  "Close All Connections": {
    "zh-Hans": "关闭所有连接",
    "zh-Hant": "關閉所有連接",
    "fa": "بستن همه اتصال‌ها",
    "ru": "Закрыть все подключения",
  },
  "URLTest All": {
    "zh-Hans": "全部测速",
    "zh-Hant": "全部測速",
    "fa": "آزمایش همه",
    "ru": "Тест всех",
  },
  "URLTest": {
    "zh-Hans": "测速",
    "zh-Hant": "測速",
    "fa": "آزمایش",
    "ru": "Тест",
  },
  "Start At Login": {
    "zh-Hans": "开机启动",
    "zh-Hant": "開機啟動",
    "fa": "اجرا هنگام ورود",
    "ru": "Запуск при входе",
  },
  "Real-time Speed": {
    "zh-Hans": "实时速率",
    "zh-Hant": "實時速率",
    "fa": "نرخ لحظه‌ای",
    "ru": "Текущая скорость",
  },
  "Disabled": {
    "zh-Hans": "已禁用",
    "zh-Hant": "已禁用",
    "fa": "غیرفعال",
    "ru": "Отключено",
  },
  "Unified": {
    "zh-Hans": "统一",
    "zh-Hant": "統一",
    "fa": "یکپارچه",
    "ru": "Единый",
  },
  "Import Remote Profile": {
    "zh-Hans": "导入远程配置",
    "zh-Hant": "導入遠程配置",
    "fa": "وارد کردن پیکربندی راه‌دور",
    "ru": "Импорт удаленной конфигурации",
  },
  "Service Setup": {
    "zh-Hans": "服务安装",
    "zh-Hant": "服務安裝",
    "fa": "راه‌اندازی سرویس",
    "ru": "Установка службы",
  },
  "Connecting...": {
    "zh-Hans": "连接中…",
    "zh-Hant": "連接中…",
    "fa": "در حال اتصال…",
    "ru": "Подключение…",
  },
  "Retry": {
    "zh-Hans": "重试",
    "zh-Hant": "重試",
    "fa": "تلاش مجدد",
    "ru": "Повторить",
  },
  "The sing-box service is not installed": {
    "zh-Hans": "sing-box 服务未安装",
    "zh-Hant": "sing-box 服務未安裝",
    "fa": "سرویس sing-box نصب نشده است",
    "ru": "Служба sing-box не установлена",
  },
  "The sing-box service is not running": {
    "zh-Hans": "sing-box 服务未运行",
    "zh-Hant": "sing-box 服務未運行",
    "fa": "سرویس sing-box در حال اجرا نیست",
    "ru": "Служба sing-box не запущена",
  },
  "Incompatible service version": {
    "zh-Hans": "服务版本不兼容",
    "zh-Hant": "服務版本不兼容",
    "fa": "نسخه سرویس ناسازگار است",
    "ru": "Несовместимая версия службы",
  },
  "Install": {
    "zh-Hans": "安装",
    "zh-Hant": "安裝",
    "fa": "نصب",
    "ru": "Установить",
  },
  "Upgrade": {
    "zh-Hans": "升级",
    "zh-Hant": "升級",
    "fa": "ارتقا",
    "ru": "Обновить",
  },
} satisfies Record<string, DesktopMessage>;

export type DesktopMessageKey = keyof typeof DESKTOP_TRANSLATIONS;

export function translateDesktop(language: DesktopLanguage, key: DesktopMessageKey): string {
  if (language === "en") {
    return key;
  }
  return DESKTOP_TRANSLATIONS[key][language] ?? key;
}

export function desktopLanguageFromLocale(locale: string): DesktopLanguage {
  const lower = locale.toLowerCase();
  if (lower.startsWith("zh")) {
    if (/hant|tw|hk|mo/.test(lower)) {
      return "zh-Hant";
    }
    return "zh-Hans";
  }
  if (lower.startsWith("fa")) {
    return "fa";
  }
  if (lower.startsWith("ru")) {
    return "ru";
  }
  return "en";
}
