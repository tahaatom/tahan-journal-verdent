import i18n from "i18next";
import { initReactI18next } from "react-i18next";

const fa = {
  translation: {
    app: {
      title: "ژورنال طهان",
      subtitle: "کرنل آریا",
    },
    nav: {
      dashboard: "داشبورد",
      journal: "ژورنال",
      tradeList: "لیست معاملات",
      fieldManager: "مدیریت فیلدها",
      pluginHealth: "سلامت پلاگین‌ها",
      backup: "بکاپ",
      settings: "تنظیمات",
    },
    placeholder: {
      mainArea: "محتوای اصلی این بخش در فازهای بعدی پیاده‌سازی می‌شود.",
    },
  },
};

i18n.use(initReactI18next).init({
  lng: "fa",
  fallbackLng: "fa",
  resources: { fa },
  interpolation: { escapeValue: false },
});

export default i18n;
